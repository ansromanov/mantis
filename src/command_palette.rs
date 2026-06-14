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
        name: "Toggle markdown render",
        action_id: "toggle_raw_markdown",
    },
    CommandEntry {
        name: "Toggle JSON pretty-print",
        action_id: "toggle_pretty_json",
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
        name: "About tree-viewer",
        action_id: "show_about",
    },
    CommandEntry {
        name: "Fold all (YAML)",
        action_id: "yaml_fold_all",
    },
    CommandEntry {
        name: "Unfold all (YAML)",
        action_id: "yaml_unfold_all",
    },
    CommandEntry {
        name: "Toggle fold at cursor (YAML)",
        action_id: "yaml_fold_toggle",
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
mod tests {
    use super::*;

    #[test]
    fn command_palette_starts_with_all_commands() {
        let p = CommandPalette::default();
        assert_eq!(p.results_len(), COMMANDS.len());
        assert_eq!(p.selected, 0);
    }

    #[test]
    fn command_palette_push_filters() {
        let mut p = CommandPalette::default();
        p.push('w');
        assert!(p.results_len() < COMMANDS.len());
        let cmd = p.selected_command().unwrap();
        assert!(cmd.name.to_lowercase().contains('w'));
    }

    #[test]
    fn command_palette_pop_restores() {
        let mut p = CommandPalette::default();
        p.push('z');
        let after_push = p.results_len();
        p.pop();
        assert_eq!(p.results_len(), COMMANDS.len());
        assert!(after_push < COMMANDS.len());
    }

    #[test]
    fn command_palette_selected_command() {
        let p = CommandPalette::default();
        let first = p.selected_command();
        assert!(first.is_some());
        assert_eq!(first.unwrap().name, COMMANDS[0].name);
    }

    #[test]
    fn command_palette_selected_index_returns_none_when_empty() {
        let mut p = CommandPalette::default();
        for c in "zzzzzzzzzzzzz".chars() {
            p.push(c);
        }
        assert_eq!(p.results_len(), 0);
        assert!(p.selected_index().is_none());
        assert!(p.selected_command().is_none());
    }

    #[test]
    fn command_palette_results_len() {
        let p = CommandPalette::default();
        assert_eq!(p.results_len(), COMMANDS.len());
    }

    #[test]
    fn command_palette_filters_by_name() {
        let mut p = CommandPalette::default();
        for c in "markdown".chars() {
            p.push(c);
        }
        assert!(p.selected_command().unwrap().name.contains("markdown"));
    }

    #[test]
    fn command_palette_filters_by_keybinding() {
        let mut p = CommandPalette::default();
        for c in "ctrl+g".chars() {
            p.push(c);
        }
        let cmd = p.selected_command().unwrap();
        assert_eq!(cmd.action_id, "toggle_git_mode");
    }
}
