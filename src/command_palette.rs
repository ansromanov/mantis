//! The Ctrl-P command palette: a fuzzy-filterable picker over the
//! palette-invokable subset of the canonical action registry, with prefix
//! routing to file search, content search, and go-to-line.
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
//!
//! **Prefix routing** lets one Ctrl-P entry point reach multiple pickers.
//! The first character of the query is checked for a prefix:
//!
//! | Prefix | Route      | Sub-picker       |
//! |--------|------------|------------------|
//! | (none) | Commands   | —                |
//! | `>`    | Commands   | (alias)          |
//! | `/`    | Files      | `SearchState`    |
//! | `#`    | Content    | `SearchState`    |
//! | `:`    | Go to Line | `GotoLineState`  |
//!
//! `@` is reserved for symbols (epic #482) and is not yet routed.
//!
//! `inapplicability_reasons` carries, per `COMMANDS` index, why a command
//! can't run right now (from `App::check_applicability`, driven by
//! `ActionSpec::applicability()`). Commands with a reason are sunk below
//! applicable ones in the empty-query `base_order` and rendered dimmed by
//! `ui::popups::command`; dispatch refuses them with a status message
//! instead of running.

use std::sync::LazyLock;

use fuzzy_matcher::skim::SkimMatcherV2;

use crate::actions::ACTIONS;
use crate::config::Keymap;
use crate::list_picker::ListPicker;
use crate::search::fuzzy_refilter;
use crate::search::{GotoLineState, SearchState};

/// Routes the command palette query to different picker modes via a
/// single-character prefix typed as the first character in the query bar.
///
/// | Prefix | Route            | Source picker      |
/// |--------|------------------|--------------------|
/// | (none) | Commands         | existing Ctrl+P    |
/// | `>`    | Commands (alias) | —                  |
/// | `/`    | File search      | `SearchState`      |
/// | `#`    | Content search   | `SearchState`      |
/// | `:`    | Go to line       | `GotoLineState`    |
///
/// `@` is reserved for symbols (epic #482) and is not yet routed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteRoute {
    Commands,
    Files,
    Content,
    GotoLine,
}

impl PaletteRoute {
    /// Maps a prefix character to a route. Returns `None` for characters
    /// that are not recognized prefixes (they become part of the query).
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            '>' => Some(Self::Commands),
            '/' => Some(Self::Files),
            '#' => Some(Self::Content),
            ':' => Some(Self::GotoLine),
            _ => None,
        }
    }

    /// Human-readable label shown in the popup title.
    pub fn label(self) -> &'static str {
        match self {
            Self::Commands => "Commands",
            Self::Files => "Files",
            Self::Content => "Content",
            Self::GotoLine => "Go to Line",
        }
    }

    /// The prefix character that activates this route.
    pub fn prefix_char(self) -> char {
        match self {
            Self::Commands => '>',
            Self::Files => '/',
            Self::Content => '#',
            Self::GotoLine => ':',
        }
    }
}

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
    /// Inapplicability reason for each command index, if any.
    pub inapplicability_reasons: Vec<Option<&'static str>>,
    /// Current routing mode determined by the first character typed.
    pub route: PaletteRoute,
    /// Sub-picker for file/content routes. `None` in Commands mode.
    pub route_search: Option<SearchState>,
    /// Sub-picker for the go-to-line route. `None` outside that route.
    pub route_goto_line: Option<GotoLineState>,
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new(
            &Keymap::default(),
            Vec::new(),
            0,
            vec![None; COMMANDS.len()],
        )
    }
}

impl CommandPalette {
    pub fn new(
        keymap: &Keymap,
        base_order: Vec<usize>,
        base_pinned: usize,
        inapplicability_reasons: Vec<Option<&'static str>>,
    ) -> Self {
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

        // Empty-query ranking keeps pinned/frequent ordering but sinks
        // inapplicable entries below applicable ones.
        let mut applicable = Vec::new();
        let mut inapplicable = Vec::new();
        let mut applicable_pinned_count = 0;
        for (idx, &i) in base_order.iter().enumerate() {
            let is_pinned = idx < base_pinned;
            let is_applicable = inapplicability_reasons.get(i).and_then(|r| *r).is_none();
            if is_applicable {
                applicable.push(i);
                if is_pinned {
                    applicable_pinned_count += 1;
                }
            } else {
                inapplicable.push(i);
            }
        }
        let base_pinned = applicable_pinned_count;
        let mut base_order = applicable;
        base_order.extend(inapplicable);

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
            inapplicability_reasons,
            route: PaletteRoute::Commands,
            route_search: None,
            route_goto_line: None,
        }
    }

    pub fn push(&mut self, c: char) {
        match self.route {
            PaletteRoute::Commands => {
                // Detect prefix on the first character of the query.
                if self.query.is_empty() {
                    if let Some(route) = PaletteRoute::from_char(c) {
                        self.route = route;
                        return;
                    }
                }
                self.query.push(c);
                self.refilter();
            }
            PaletteRoute::Files | PaletteRoute::Content => {
                if let Some(ref mut s) = self.route_search {
                    s.push(c);
                }
            }
            PaletteRoute::GotoLine => {
                if let Some(ref mut g) = self.route_goto_line {
                    g.push(c);
                }
            }
        }
    }

    pub fn pop(&mut self) {
        match self.route {
            PaletteRoute::Commands => {
                self.query.pop();
                self.refilter();
            }
            PaletteRoute::Files | PaletteRoute::Content => {
                match self.route_search {
                    Some(ref mut s) => {
                        s.pop();
                        if s.query.is_empty() {
                            self.route_search = None;
                            self.route = PaletteRoute::Commands;
                            self.selected = 0;
                            self.filtered = self.base_order.clone();
                            self.match_positions = vec![Vec::new(); self.filtered.len()];
                        }
                    }
                    None => {
                        // No sub-picker yet (prefix just typed, no chars after
                        // it); return to commands mode.
                        self.route = PaletteRoute::Commands;
                        self.selected = 0;
                        self.filtered = self.base_order.clone();
                        self.match_positions = vec![Vec::new(); self.filtered.len()];
                    }
                }
            }
            PaletteRoute::GotoLine => match self.route_goto_line {
                Some(ref mut g) => {
                    g.pop();
                    if g.query.is_empty() {
                        self.route_goto_line = None;
                        self.route = PaletteRoute::Commands;
                        self.selected = 0;
                        self.filtered = self.base_order.clone();
                        self.match_positions = vec![Vec::new(); self.filtered.len()];
                    }
                }
                None => {
                    self.route = PaletteRoute::Commands;
                    self.selected = 0;
                    self.filtered = self.base_order.clone();
                    self.match_positions = vec![Vec::new(); self.filtered.len()];
                }
            },
        }
    }

    pub fn results_len(&self) -> usize {
        match self.route {
            PaletteRoute::Commands => self.filtered.len(),
            PaletteRoute::Files | PaletteRoute::Content => {
                self.route_search.as_ref().map_or(0, |s| s.results_len())
            }
            PaletteRoute::GotoLine => self.route_goto_line.as_ref().map_or(0, |g| g.results_len()),
        }
    }

    /// Returns `true` when the effective query (commands or routed) is empty.
    pub fn is_query_empty(&self) -> bool {
        match self.route {
            PaletteRoute::Commands => self.query.is_empty(),
            PaletteRoute::Files | PaletteRoute::Content => self
                .route_search
                .as_ref()
                .is_none_or(|s| s.query.is_empty()),
            PaletteRoute::GotoLine => self
                .route_goto_line
                .as_ref()
                .is_none_or(|g| g.query.is_empty()),
        }
    }

    /// Returns the effective query string for the active route.
    pub fn active_query(&self) -> &str {
        match self.route {
            PaletteRoute::Commands => &self.query,
            PaletteRoute::Files | PaletteRoute::Content => {
                static EMPTY: &str = "";
                self.route_search
                    .as_ref()
                    .map(|s| s.query.as_str())
                    .unwrap_or(EMPTY)
            }
            PaletteRoute::GotoLine => {
                static EMPTY: &str = "";
                self.route_goto_line
                    .as_ref()
                    .map(|g| g.query.as_str())
                    .unwrap_or(EMPTY)
            }
        }
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
        self.is_query_empty()
    }
    fn results_len(&self) -> usize {
        self.results_len()
    }
    fn selected(&self) -> usize {
        match self.route {
            PaletteRoute::Commands => self.selected,
            PaletteRoute::Files | PaletteRoute::Content => {
                self.route_search.as_ref().map_or(0, |s| s.selected)
            }
            PaletteRoute::GotoLine => 0,
        }
    }
    fn set_selected(&mut self, i: usize) {
        match self.route {
            PaletteRoute::Commands => self.selected = i,
            PaletteRoute::Files | PaletteRoute::Content => {
                if let Some(ref mut s) = self.route_search {
                    s.selected = i;
                }
            }
            PaletteRoute::GotoLine => {
                // GotoLine has no selectable list; ignore.
                let _ = i;
            }
        }
    }
}

#[cfg(test)]
#[path = "command_palette_test.rs"]
mod tests;
