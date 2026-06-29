//! Loading, parsing, and saving of the `mantis.toml` configuration.
//!
//! Two layers: the **embedded defaults** (the fully-commented `mantis.toml` template
//! baked into the binary) supply every value, and the user's `mantis.toml` overrides
//! only the keys it sets — serde's `#[serde(default)]` merges the two. On launch
//! a read-only `mantis.default.toml` reference is (re)written next to the user config
//! whenever it is missing or stale, so an upgrade always refreshes the documented
//! option catalogue without ever touching the user's own file. The user `mantis.toml`
//! is created once as a minimal stub and from then on is only written by `save`,
//! which emits a *sparse* override file (changed-from-default keys only).
//!
//! Sub-modules:
//! - `types` — `Config` and grouped sub-configs (`TreeConfig`, `ContentConfig`, …)
//!   with serde defaults and one-time legacy-field migration.
//! - `keymap` — `KeyBinding`, `Keymap`, parsing (`parse_binding`, `bind`),
//!   and the `pressed` matcher.
//! - `validate` — schema validation for unknown-key detection.
//!
//! `load` locates and deserializes the config, returning any validation warning
//! rather than failing the launch; `save` writes the current settings back.
//! Keep new fields in `types` in sync with the defaults so round-tripping
//! a saved config is lossless.

mod keymap;
mod types;
mod validate;

// Used in both test and non-test code.
pub use keymap::{pressed, Keymap};
pub use types::{Config, StatusBarConfig};
// Used by name only in test code (struct literals in *_test helpers).
#[cfg_attr(not(test), allow(unused_imports))]
pub use types::{GitConfig, GitDiffConfig, TreeConfig};
// Only referenced in doc links or via field access — never named in code.
#[allow(unused_imports)]
pub use keymap::KeyBinding;
#[allow(unused_imports)]
pub use types::{ContentConfig, SearchConfig};
// Internal helpers used in tests.
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use keymap::{bind, parse_binding};

use std::fs;
use std::path::{Path, PathBuf};

/// Loads config for the given view root. A project-local `mantis.toml` found in
/// the root or any ancestor takes precedence over the global config; this lets
/// a repo ship its own defaults. On first run it seeds a minimal user config and
/// the bundled themes/plugins, and on every run refreshes the `mantis.default.toml`
/// reference; it never overwrites an existing user config. Returns the loaded
/// config, the path it was loaded from
/// (so that live changes are saved back to the same file), and a warning
/// describing the first malformed config encountered, if any, so the caller can
/// tell the user their config was ignored instead of failing silently.
pub fn load(root: &Path) -> (Config, Option<PathBuf>, Option<String>) {
    migrate_legacy_config();
    let global = global_config_path();
    if let Some(ref path) = global {
        init_config_dir(path);
    }
    let mut error = None;
    for path in config_paths(root) {
        let Ok(s) = fs::read_to_string(&path) else {
            continue; // missing or unreadable: try the next candidate
        };
        match toml::from_str::<Config>(&s) {
            Ok(mut config) => {
                config.migrate_legacy_flat_fields();
                config.migrate_legacy_git_fields();
                // The config parsed, but `#[serde(default)]` silently ignores
                // unknown keys. Flag them (with nearest-match hints) so typos
                // don't get dropped without a word. A higher-precedence parse
                // failure already recorded above takes priority.
                if error.is_none() {
                    let unknown = validate::validate_keys(&s);
                    if !unknown.is_empty() {
                        error = Some(format!("{}: {}", path.display(), unknown.join("; ")));
                    }
                }
                return (config, Some(path), error);
            }
            // Record the first malformed config but keep falling back so a valid
            // lower-precedence file (e.g. the global config) can still load.
            Err(e) if error.is_none() => {
                error = Some(format!("{}: {e}", path.display()));
            }
            Err(_) => {}
        }
    }
    (Config::default(), global, error)
}

/// Writes `config` back to the user's `path` as a *sparse* override file: only
/// the keys whose value differs from the built-in defaults are written, so the
/// user config stays small and readable instead of growing into a full dump of
/// every setting.
pub fn save(config: &Config, path: &Path) -> std::io::Result<()> {
    fs::write(path, sparse_toml(config))
}

/// Serialises `config` keeping only the top-level keys whose value differs from
/// `Config::default()`. This keeps the user's `mantis.toml` a minimal override file:
/// untouched settings fall through to the embedded defaults rather than being
/// pinned to their current value (which would also mask future default changes).
pub fn sparse_toml(config: &Config) -> String {
    let current = toml::Value::try_from(config);
    let default = toml::Value::try_from(Config::default());
    let (Ok(toml::Value::Table(cur)), Ok(toml::Value::Table(def))) = (current, default) else {
        // Serialisation should never fail for our own type; fall back to a full
        // dump rather than losing the user's settings.
        return toml::to_string_pretty(config).unwrap_or_default();
    };
    let mut out = toml::map::Map::new();
    for (k, v) in &cur {
        if def.get(k) != Some(v) {
            out.insert(k.clone(), v.clone());
        }
    }
    toml::to_string_pretty(&toml::Value::Table(out)).unwrap_or_default()
}

/// Prepares the global config directory on launch. The fully-commented default
/// reference (`mantis.default.toml`) is refreshed whenever it is missing or stale
/// (i.e. after an upgrade), so users always have an up-to-date catalogue of every
/// option. The user's own `mantis.toml` is **never** overwritten: it is created once,
/// as a minimal stub, only when absent. Bundled themes and plugins are seeded on
/// that same first run.
fn init_config_dir(user_path: &Path) {
    let Some(dir) = user_path.parent() else {
        return;
    };
    let _ = fs::create_dir_all(dir);
    refresh_default_reference(dir);
    if !user_path.exists() {
        let _ = fs::write(user_path, USER_CONFIG_STUB);
        crate::theme::install_embedded_themes();
        crate::plugin::install_bundled_plugins();
    }
}

/// Writes the embedded fully-commented template to `{dir}/mantis.default.toml`, but
/// only when the file is missing or its contents differ from the embedded
/// version (the upgrade case). Returns whether the file was (re)written. This is
/// a read-only reference for users; `mantis` itself reads values from the embedded
/// defaults, never from this file.
fn refresh_default_reference(dir: &Path) -> bool {
    let path = dir.join(DEFAULT_REFERENCE_NAME);
    if fs::read_to_string(&path).ok().as_deref() == Some(DEFAULT_CONFIG_TEMPLATE) {
        return false;
    }
    fs::write(&path, DEFAULT_CONFIG_TEMPLATE).is_ok()
}

/// The embedded, fully-commented default configuration. Source of truth for both
/// default values and the on-disk `mantis.default.toml` reference.
const DEFAULT_CONFIG_TEMPLATE: &str = include_str!("../../mantis.toml");

/// Filename of the read-only default reference written next to the user config.
const DEFAULT_REFERENCE_NAME: &str = "mantis.default.toml";

/// Minimal first-run user config. Kept deliberately tiny: the user adds only the
/// overrides they want, and consults `mantis.default.toml` for the full option list.
const USER_CONFIG_STUB: &str = "\
# mantis user config -- your overrides only.
#
# This file is never modified by upgrades. Add only the settings you want to
# change; everything else falls back to the built-in defaults.
#
# See mantis.default.toml in this directory (refreshed on every upgrade) for the
# full, commented list of available options.
";

/// Candidate config paths in precedence order: project-local (`mantis.toml` in the
/// root and each ancestor), then the global config.
fn config_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = root.ancestors().map(|d| d.join("mantis.toml")).collect();
    if let Some(global) = global_config_path() {
        paths.push(global);
    }
    paths
}

fn global_config_path() -> Option<PathBuf> {
    dirs_next()?.join("mantis.toml").into()
}

fn dirs_next() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("mantis"))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|base| base.join("mantis"))
    }
}

/// One-time migration from the legacy `tree-viewer` config directory to `mantis`.
/// Called once at startup before any config resolution. If the new directory
/// (`~/.config/mantis/`) does not exist but the old (`~/.config/tree-viewer/`)
/// does, the old directory is renamed to the new name. Inside it, `tv.toml` is
/// renamed to `mantis.toml` and `tv.default.toml` to `mantis.default.toml`.
/// Best-effort: never destroys data on failure.
fn migrate_legacy_config() {
    let old_dir = legacy_dirs_next();
    let new_dir = dirs_next();
    let (Some(old), Some(new)) = (old_dir, new_dir) else {
        return;
    };
    if new.exists() || !old.exists() {
        return;
    }
    // Rename config files inside the old directory before moving the dir.
    for (old_name, new_name) in [
        ("tv.toml", "mantis.toml"),
        ("tv.default.toml", "mantis.default.toml"),
    ] {
        let old_file = old.join(old_name);
        let new_file = old.join(new_name);
        if old_file.exists() {
            let _ = fs::rename(&old_file, &new_file);
        }
    }
    // Rename the entire directory.
    let _ = fs::rename(&old, &new);
}

/// Returns the legacy config directory path (`tree-viewer`). Used only for
/// one-time migration.
fn legacy_dirs_next() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("tree-viewer"))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|base| base.join("tree-viewer"))
    }
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod tests;
