//! The Ctrl-P command palette: a static registry of named actions and a
//! fuzzy-filterable picker over them.
//!
//! `COMMANDS` is the canonical list of every palette-invokable action, each
//! pairing a human-readable `name` with a stable `action_id` string. The
//! `CommandPalette` picker holds the query, the scored-and-filtered subset, and
//! the selected index, refreshing via `SkimMatcherV2` as the user types.
//! Selecting an entry yields its `action_id`, which `app::key_handlers::editor`
//! dispatches to the matching `App` method - so the palette and direct
//! keybindings stay a single source of truth. Add new commands here and wire the
//! `action_id` into that dispatcher.

use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

use crate::config::Keymap;

pub struct CommandEntry {
    pub name: &'static str,
    pub action_id: &'static str,
}

pub static COMMANDS: &[CommandEntry] = &[
    CommandEntry {
        name: "Toggle help",
        action_id: "toggle_help",
    },
    CommandEntry {
        name: "Toggle hidden files",
        action_id: "toggle_hidden",
    },
    CommandEntry {
        name: "Open file search",
        action_id: "open_file_search",
    },
    CommandEntry {
        name: "Open content search",
        action_id: "open_content_search",
    },
    CommandEntry {
        name: "Reload",
        action_id: "reload",
    },
    CommandEntry {
        name: "Open file history",
        action_id: "open_file_history",
    },
    CommandEntry {
        name: "Open theme picker",
        action_id: "open_theme_picker",
    },
    CommandEntry {
        name: "Toggle git mode",
        action_id: "toggle_git_mode",
    },
    CommandEntry {
        name: "Toggle git flat mode",
        action_id: "toggle_git_flat",
    },
    CommandEntry {
        name: "Toggle word wrap",
        action_id: "toggle_word_wrap",
    },
    CommandEntry {
        name: "Toggle line numbers",
        action_id: "toggle_line_numbers",
    },
    CommandEntry {
        name: "Toggle markdown render",
        action_id: "toggle_raw_markdown",
    },
    CommandEntry {
        name: "Toggle JSON pretty-print",
        action_id: "toggle_pretty_json",
    },
    CommandEntry {
        name: "Visual line mode (select lines, blame)",
        action_id: "toggle_visual_line",
    },
    CommandEntry {
        name: "Toggle side-by-side diff",
        action_id: "toggle_diff_side_by_side",
    },
    CommandEntry {
        name: "Open in editor",
        action_id: "open_in_editor",
    },
    CommandEntry {
        name: "Open config in editor",
        action_id: "open_config_in_editor",
    },
    CommandEntry {
        name: "Toggle auto watch (reload on file change)",
        action_id: "toggle_watch",
    },
    CommandEntry {
        name: "About tree-viewer",
        action_id: "show_about",
    },
    CommandEntry {
        name: "Fold all",
        action_id: "fold_all",
    },
    CommandEntry {
        name: "Unfold all",
        action_id: "unfold_all",
    },
    CommandEntry {
        name: "Toggle fold at cursor",
        action_id: "fold_toggle",
    },
    CommandEntry {
        name: "Copy absolute path",
        action_id: "copy_path",
    },
    CommandEntry {
        name: "Copy relative path",
        action_id: "copy_relative_path",
    },
    CommandEntry {
        name: "Blame active line",
        action_id: "blame_line",
    },
    CommandEntry {
        name: "Open plugin manager",
        action_id: "open_plugin_picker",
    },
    CommandEntry {
        name: "Collapse all directories",
        action_id: "tree_collapse_all",
    },
    CommandEntry {
        name: "Expand all directories",
        action_id: "tree_expand_all",
    },
    CommandEntry {
        name: "Go to line",
        action_id: "go_to_line",
    },
    CommandEntry {
        name: "Go up one directory",
        action_id: "tree_up_dir",
    },
];

pub struct CommandPalette {
    pub query: String,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub binding_labels: Vec<String>,
    matcher: SkimMatcherV2,
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new(&Keymap::default())
    }
}

impl CommandPalette {
    pub fn new(keymap: &Keymap) -> Self {
        let binding_labels = COMMANDS
            .iter()
            .map(|cmd| keymap.label_for_action(cmd.action_id))
            .collect();
        let filtered = (0..COMMANDS.len()).collect();
        CommandPalette {
            query: String::new(),
            filtered,
            selected: 0,
            binding_labels,
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
            self.filtered = (0..COMMANDS.len()).collect();
            return;
        }
        let hay = |i: usize| format!("{} {}", COMMANDS[i].name, self.binding_labels[i]);
        let mut scored: Vec<(usize, i64)> = (0..COMMANDS.len())
            .filter_map(|i| {
                self.matcher
                    .fuzzy_match(&hay(i), &self.query)
                    .map(|s| (i, s))
            })
            .collect();
        scored.sort_by_key(|(_, s)| std::cmp::Reverse(*s));
        self.filtered = scored.into_iter().map(|(i, _)| i).collect();
    }
}

#[cfg(test)]
#[path = "command_palette_test.rs"]
mod tests;
