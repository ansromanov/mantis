//! Key handling for the tree filter bar and its expansion bookkeeping.

use std::collections::HashSet;
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};

use crate::config::static_keys;
use crate::list_picker::{handle_list_picker_key, OverlayKey};

use super::super::App;

impl App {
    /// Handles keyboard input while the inline tree filter is open.
    /// Up/Down/PageUp/PageDown navigate between matches; other keys
    /// (typing, Backspace, Esc, Enter) go through the generic picker.
    pub(crate) fn handle_tree_filter_key(&mut self, key: KeyEvent) {
        if self.tree_filter.is_none() {
            return;
        }
        let page = (self.geo.tree_area.height as usize).max(1) as isize;
        if key.code == KeyCode::Up {
            self.move_tree_filter_selection(-1);
            return;
        }
        if key.code == KeyCode::Down {
            self.move_tree_filter_selection(1);
            return;
        }
        if static_keys::is_page_up(&key) {
            self.move_tree_filter_selection(-page);
            return;
        }
        if static_keys::is_page_down(&key) {
            self.move_tree_filter_selection(page);
            return;
        }
        let Some(f) = self.tree_filter.as_mut() else {
            return;
        };
        let is_query_edit = matches!(key.code, KeyCode::Char(_) | KeyCode::Backspace);
        match handle_list_picker_key(f, &key) {
            OverlayKey::Activate => {
                self.restore_tree_filter_expansion();
                self.tree_filter = None;
                self.activate_selected_and_focus();
            }
            OverlayKey::Close => {
                self.restore_tree_filter_expansion();
                self.tree_filter = None;
            }
            OverlayKey::Handled => {
                if is_query_edit {
                    self.sync_tree_filter_expansion();
                }
                self.move_tree_filter_selection_to_first_match();
            }
            _ => {}
        }
    }

    /// Auto-expands the ancestor chain of every full-tree match for the
    /// current filter query so matches inside collapsed directories become
    /// navigable, and rebuilds the visible node list to reflect it. Matching
    /// walks the whole tree (not just `self.nodes`, which only contains
    /// currently-visible entries) via a cache built on the filter's first
    /// non-empty query. Restores the pre-filter expansion state instead when
    /// the query becomes empty (e.g. backspacing it all out).
    fn sync_tree_filter_expansion(&mut self) {
        let Some(filter) = self.tree_filter.as_ref() else {
            return;
        };
        if filter.is_empty() {
            self.restore_tree_filter_expansion();
            return;
        }

        if self
            .tree_filter
            .as_ref()
            .is_some_and(|f| f.full_paths_cache.is_none())
        {
            let mut all =
                crate::tree::collect_all_dirs(&self.root, self.show_hidden, self.ignore_gitignore);
            all.extend(crate::tree::collect_all_files(
                &self.root,
                self.show_hidden,
                self.ignore_gitignore,
            ));
            // Pair each path with its lowercased file name up front so the
            // per-keystroke matching below is allocation-free.
            let all: Vec<(PathBuf, String)> = all
                .into_iter()
                .map(|p| {
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    (p, name)
                })
                .collect();
            if let Some(f) = self.tree_filter.as_mut() {
                f.full_paths_cache = Some(all);
            }
        }

        // Take ownership of the cache first (requires mutable access) so we
        // don't hold an immutable borrow across it.
        let paths = self
            .tree_filter
            .as_mut()
            .and_then(|f| f.full_paths_cache.take())
            .unwrap_or_default();

        let Some(filter) = self.tree_filter.as_ref() else {
            return;
        };

        let mut to_expand: HashSet<PathBuf> = HashSet::new();
        for (path, name) in &paths {
            // `name` is already lowercased in the cache; `matches_name` is
            // case-insensitive so matching against a lowercased name is fine.
            if !filter.matches_name(name) {
                continue;
            }
            let mut ancestor = path.parent();
            while let Some(p) = ancestor {
                if p == self.root {
                    break;
                }
                // An earlier match already recorded this ancestor, and that
                // walk continued to the root, so the rest of this chain is
                // recorded too.
                if to_expand.contains(p) {
                    break;
                }
                if !self.expanded.contains(p) {
                    to_expand.insert(p.to_path_buf());
                }
                ancestor = p.parent();
            }
        }

        if let Some(f) = self.tree_filter.as_mut() {
            f.full_paths_cache = Some(paths);
        }

        if !to_expand.is_empty() {
            if self
                .tree_filter
                .as_ref()
                .is_some_and(|f| f.saved_expanded.is_none())
            {
                let snapshot = self.expanded.clone();
                if let Some(f) = self.tree_filter.as_mut() {
                    f.saved_expanded = Some(snapshot);
                }
            }
            self.expanded.extend(to_expand);
            self.rebuild(false);
        }
    }

    /// Restores `expanded` to the snapshot taken before the tree filter
    /// auto-expanded any directories, if one was taken. No-op when the
    /// filter never triggered an auto-expansion (e.g. all matches were
    /// already visible), or when the snapshot matches the current state.
    fn restore_tree_filter_expansion(&mut self) {
        let saved = self
            .tree_filter
            .as_mut()
            .and_then(|f| f.saved_expanded.take());
        if let Some(saved) = saved {
            if saved != self.expanded {
                self.expanded = saved;
                self.rebuild(false);
            }
        }
    }

    /// Move the tree-filter selection by `delta` rows within the filtered
    /// match set (`tree_visible_indices`), clamping to its bounds. Does
    /// nothing when no filter set is recorded or the set is empty.
    fn move_tree_filter_selection(&mut self, delta: isize) {
        let visible: Vec<usize> = match self.tree_visible_indices.as_ref() {
            Some(v) if !v.is_empty() => v.clone(),
            _ => return,
        };
        let cur_pos = visible
            .iter()
            .position(|&i| i == self.tree_selected)
            .unwrap_or(0);
        let new_pos = (cur_pos as isize + delta).clamp(0, visible.len() as isize - 1) as usize;
        if let Some(&node_idx) = visible.get(new_pos) {
            self.tree_selected = node_idx;
            self.scroll_tree_into_view();
        }
    }

    /// Moves `tree_selected` to the first visible match index when the inline
    /// tree filter is active. If the query is empty or no node matches, the
    /// selection stays at index 0 (the root).
    fn move_tree_filter_selection_to_first_match(&mut self) {
        let Some(ref filter) = self.tree_filter else {
            return;
        };
        if filter.is_empty() {
            self.tree_selected = 0;
            self.scroll_tree_into_view();
            return;
        }
        let first_match = self.nodes.iter().position(|n| filter.matches_name(&n.name));
        self.tree_selected = first_match.unwrap_or(0);
        self.scroll_tree_into_view();
    }
}
