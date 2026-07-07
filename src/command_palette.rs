//! The Ctrl-P command palette: a fuzzy-filterable picker over the
//! palette-invokable subset of the canonical action registry.
//!
//! `COMMANDS` is derived from `crate::actions::ACTIONS`, keeping only entries
//! with `palette: Some(_)`, so this module no longer hand-maintains its own
//! parallel action list. Each entry pairs a human-readable `name` with a
//! stable `action_id` string. The `CommandPalette` picker holds the query,
//! the scored-and-filtered subset, and the selected index, refreshing via
//! `SkimMatcherV2` as the user types. Selecting an entry yields its
//! `action_id`, which `app::key_handlers::editor` dispatches to the matching
//! `App` method - so the palette and direct keybindings share one set of
//! canonical ids. Add new commands to `ACTIONS` (not here) and wire the
//! `action_id` into that dispatcher.

use std::sync::LazyLock;

use fuzzy_matcher::skim::SkimMatcherV2;

use crate::actions::ACTIONS;
use crate::config::Keymap;
use crate::list_picker::ListPicker;
use crate::search::fuzzy_refilter;

pub struct CommandEntry {
    pub name: &'static str,
    pub action_id: &'static str,
    pub category: Option<&'static str>,
    pub description: Option<&'static str>,
}

/// Palette-invokable actions, in `ACTIONS` order.
pub static COMMANDS: LazyLock<Vec<CommandEntry>> = LazyLock::new(|| {
    ACTIONS
        .iter()
        .filter_map(|a| {
            a.palette.map(|name| CommandEntry {
                name,
                action_id: a.id,
                category: a.category,
                description: a.description,
            })
        })
        .collect()
});

pub struct CommandPalette {
    pub query: String,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub binding_labels: Vec<String>,
    /// Index order used when the query is empty. Always a permutation of
    /// `0..COMMANDS.len()`. Built by [`ranked_base_order`] from usage stats.
    pub base_order: Vec<usize>,
    /// How many of the first entries in `base_order` are pinned (recent +
    /// frequent). Used by the UI to show a star prefix for pinned commands
    /// when the query is empty.
    pub base_pinned: usize,
    /// Fuzzy match character positions for each entry in `filtered`, used by
    /// `draw_command_palette` to highlight matched characters. Empty when the
    /// query is empty. Each inner vec corresponds to `filtered[i]`.
    pub match_positions: Vec<Vec<usize>>,
    matcher: SkimMatcherV2,
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new(&Keymap::default(), Vec::new(), 0)
    }
}

impl CommandPalette {
    pub fn new(keymap: &Keymap, base_order: Vec<usize>, base_pinned: usize) -> Self {
        let binding_labels = COMMANDS
            .iter()
            .map(|cmd| keymap.label_for_action(cmd.action_id))
            .collect();
        // Sanitise base_order: drop out-of-range indices, de-duplicate, then
        // append any missing indices in natural order so it is always a valid
        // permutation of 0..COMMANDS.len().
        let base_order = if base_order.is_empty() {
            (0..COMMANDS.len()).collect()
        } else {
            let mut seen = vec![false; COMMANDS.len()];
            let mut order: Vec<usize> = Vec::with_capacity(COMMANDS.len());
            for &i in &base_order {
                if i < COMMANDS.len() && !seen[i] {
                    seen[i] = true;
                    order.push(i);
                }
            }
            for (i, s) in seen.iter_mut().enumerate() {
                if !*s {
                    *s = true;
                    order.push(i);
                }
            }
            order
        };
        let len = base_order.len();
        CommandPalette {
            query: String::new(),
            filtered: base_order.clone(),
            selected: 0,
            binding_labels,
            base_order,
            base_pinned,
            match_positions: vec![Vec::new(); len],
            matcher: SkimMatcherV2::default(),
        }
    }

    pub fn push(&mut self, c: char) {
        self.query.push(c);
        self.refilter();
    }

    pub fn pop(&mut self) {
        self.query.pop();
        self.refilter();
    }

    pub fn results_len(&self) -> usize {
        self.filtered.len()
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.filtered.get(self.selected).copied()
    }

    pub fn selected_command(&self) -> Option<&'static CommandEntry> {
        self.selected_index().map(|i| &COMMANDS[i])
    }

    fn refilter(&mut self) {
        self.selected = 0;
        if self.query.is_empty() {
            self.filtered = self.base_order.clone();
            self.match_positions = vec![Vec::new(); self.filtered.len()];
            return;
        }
        let binding_labels = &self.binding_labels;
        let indices: Vec<usize> = (0..COMMANDS.len()).collect();
        let results = fuzzy_refilter(
            &indices,
            &self.matcher,
            &self.query,
            |&i| {
                let cmd = &COMMANDS[i];
                let mut haystack = cmd.category.map(|c| format!("{c}: ")).unwrap_or_default();
                haystack.push_str(cmd.name);
                if !binding_labels[i].is_empty() {
                    haystack.push_str(&format!(" [{}]", binding_labels[i]));
                }
                std::borrow::Cow::Owned(haystack)
            },
            true,
        );
        self.filtered = results.iter().map(|&(i, _)| i).collect();
        self.match_positions = results
            .into_iter()
            .map(|(_, positions)| positions)
            .collect();
    }
}

/// Build the empty-query display order: `[last_used] ++ top_used(frequent) ++ rest`,
/// de-duplicated, with every remaining command appended in source order.
/// Returns the ordered indices and the number of pinned entries.
/// `pin_recent`/`frequent` come from config. Unknown action_ids (e.g. a renamed
/// command still in an old usage file) are ignored.
pub fn ranked_base_order(
    usage: &crate::command_usage::UsageStats,
    pin_recent: bool,
    frequent: usize,
) -> (Vec<usize>, usize) {
    let index_of = |id: &str| COMMANDS.iter().position(|c| c.action_id == id);
    let mut order: Vec<usize> = Vec::with_capacity(COMMANDS.len());
    let mut seen = vec![false; COMMANDS.len()];
    let mut pinned = 0;

    let mut push = |i: usize, is_pinned: bool| {
        if !seen[i] {
            seen[i] = true;
            order.push(i);
            if is_pinned {
                pinned += 1;
            }
        }
    };

    if pin_recent {
        if let Some(i) = usage.last_used().and_then(index_of) {
            push(i, true);
        }
    }
    for id in usage.top_used(frequent) {
        if let Some(i) = index_of(id) {
            push(i, true);
        }
    }
    for i in 0..COMMANDS.len() {
        push(i, false);
    }
    (order, pinned)
}

impl ListPicker for CommandPalette {
    fn query_push(&mut self, c: char) {
        self.push(c);
    }
    fn query_pop(&mut self) {
        self.pop();
    }
    fn query_is_empty(&self) -> bool {
        self.query.is_empty()
    }
    fn results_len(&self) -> usize {
        self.results_len()
    }
    fn selected(&self) -> usize {
        self.selected
    }
    fn set_selected(&mut self, i: usize) {
        self.selected = i;
    }
}

#[cfg(test)]
#[path = "command_palette_test.rs"]
mod tests;
