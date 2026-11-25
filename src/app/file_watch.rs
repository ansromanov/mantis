//! Filesystem watchers for `App`: the open file, the tree root and the config file.

use std::path::{Path, PathBuf};

use notify::{EventKind, RecursiveMode, Watcher};

use super::App;

impl App {
    /// Sets up a filesystem watcher on the parent directory of `path` so that
    /// `drain_file_watch` can detect external edits. Clears any previous watch.
    /// Watches the parent directory (not the file) to catch atomic-save renames.
    pub(super) fn set_file_watch(&mut self, path: Option<&Path>) {
        self.file_watcher = None;
        self.file_watch_rx = None;
        self.file_watch_path = None;
        let Some(p) = path else { return };
        // Watch the parent directory rather than the file itself so that
        // atomic-save editors (those that write a temp file and rename it over
        // the original) still trigger events after the inode is replaced.
        let Some(dir) = p.parent() else { return };
        let (tx, rx) = std::sync::mpsc::channel();
        let Ok(mut watcher) = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        }) else {
            return;
        };
        if watcher.watch(dir, RecursiveMode::NonRecursive).is_ok() {
            self.file_watcher = Some(watcher);
            self.file_watch_rx = Some(rx);
            self.file_watch_path = Some(p.to_path_buf());
        }
    }

    /// Installs a recursive filesystem watcher on the view root so that
    /// `drain_root_watch` can detect tree changes (files added/removed, git
    /// status changes from edits anywhere in the repo) and drive a debounced
    /// reload instead of a blind periodic one. Best-effort: if the watcher
    /// cannot be installed (e.g. the OS hits a watch-descriptor limit on a very
    /// large tree) the field stays `None` and `tick` falls back to the timer.
    pub fn watch_root(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        let Ok(mut watcher) = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        }) else {
            return;
        };
        if watcher.watch(&self.root, RecursiveMode::Recursive).is_ok() {
            self.root_watcher = Some(watcher);
            self.root_watch_rx = Some(rx);
        }
    }

    /// Installs a non-recursive watcher on the parent directory of the config
    /// file so edits to `mantis.toml` are detected and hot-reloaded. Best-effort:
    /// if the watcher cannot be installed (e.g. the directory does not exist) the
    /// fields stay `None` and no live-reload happens — the user simply does not
    /// get the feature.
    pub fn install_config_watcher(&mut self) {
        let Some(ref path) = self.config_path else {
            return;
        };
        let Some(dir) = path.parent() else {
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let Ok(mut watcher) = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        }) else {
            return;
        };
        if watcher.watch(dir, RecursiveMode::NonRecursive).is_ok() {
            self.config_watcher = Some(watcher);
            self.config_watch_rx = Some(rx);
        }
    }

    /// Drains root-watch events and returns whether the tree changed plus the
    /// newest qualifying file path. Directory events still invalidate the tree;
    /// only file paths can become follow-mode candidates.
    pub(super) fn drain_root_watch(&self) -> (bool, Option<PathBuf>) {
        let Some(rx) = &self.root_watch_rx else {
            return (false, None);
        };
        let mut tree_changed = false;
        let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
        // mantis's own state/config dirs may live under the root (e.g. when
        // run from `~`); events from our own session writes must not trigger
        // a reload, or the reload's session save re-triggers the watcher.
        let mut own_dirs: Option<Vec<PathBuf>> = None;
        while let Ok(res) = rx.try_recv() {
            if let Ok(evt) = res {
                if matches!(
                    evt.kind,
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                ) {
                    for path in evt.paths {
                        if !path.starts_with(&self.root) || path.starts_with(self.root.join(".git"))
                        {
                            continue;
                        }
                        let own = own_dirs.get_or_insert_with(|| {
                            let mut dirs: Vec<PathBuf> =
                                crate::session::state_dir().into_iter().collect();
                            dirs.extend(
                                self.config_path
                                    .as_ref()
                                    .and_then(|p| p.parent().map(PathBuf::from)),
                            );
                            dirs
                        });
                        if own.iter().any(|d| path.starts_with(d)) {
                            continue;
                        }
                        tree_changed = true;
                        if !path.is_file() {
                            continue;
                        }
                        let modified = std::fs::metadata(&path)
                            .and_then(|metadata| metadata.modified())
                            .unwrap_or(std::time::UNIX_EPOCH);
                        if newest.as_ref().is_none_or(|(time, _)| modified > *time) {
                            newest = Some((modified, path));
                        }
                    }
                }
            }
        }
        (tree_changed, newest.map(|(_, path)| path))
    }

    /// Drains all pending file-watch events and returns `true` if the watched
    /// file was modified, created, or deleted since the last check.
    pub(super) fn drain_file_watch(&self) -> bool {
        let (Some(rx), Some(watched)) = (&self.file_watch_rx, &self.file_watch_path) else {
            return false;
        };
        let mut changed = false;
        while let Ok(res) = rx.try_recv() {
            if let Ok(evt) = res {
                let affects_watched = evt.paths.iter().any(|p| p == watched);
                if affects_watched
                    && matches!(
                        evt.kind,
                        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
                    )
                {
                    changed = true;
                }
            }
        }
        changed
    }

    /// Drains all pending config-watch events and returns `true` if the config
    /// file was modified, created, or deleted since the last check.
    pub(super) fn drain_config_watch(&self) -> bool {
        let (Some(rx), Some(watched)) = (&self.config_watch_rx, &self.config_path) else {
            return false;
        };
        let mut changed = false;
        while let Ok(res) = rx.try_recv() {
            if let Ok(evt) = res {
                let affects_watched = evt.paths.iter().any(|p| p == watched);
                if affects_watched
                    && matches!(
                        evt.kind,
                        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
                    )
                {
                    changed = true;
                }
            }
        }
        changed
    }
}
