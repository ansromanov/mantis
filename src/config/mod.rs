//! Loading, parsing, and saving of the `mantis.toml` configuration.
//!
//! Two layers: the **embedded defaults** (the fully-commented `mantis.toml` template
//! baked into the binary) supply every value, and the user's `mantis.toml` overrides
//! only the keys it sets — serde's `#[serde(default)]` merges the two. On launch
//! a read-only `mantis.default.toml` reference is (re)written next to the user config
//! whenever it is missing or stale, so an upgrade always refreshes the documented
//! option catalogue without ever touching the user's own file. The user `mantis.toml`
//! is created once as a minimal stub and from then on is only written by
//! `save_changes`, which edits the existing file in place (via `toml_edit`) so
//! comments, ordering, and untouched keys survive -- only the settings changed
//! at runtime are rewritten. `sparse_toml` (changed-from-default keys only) is
//! used when there is no file to edit yet.
//!
//! Sub-modules:
//! - `types` — `Config` and grouped sub-configs (`TreeConfig`, `ContentConfig`,
//!   `UiConfig`, …) with serde defaults and one-time legacy-field migration.
//! - `keymap` — `KeyBinding`, `Keymap`, parsing (`parse_binding`, `bind`),
//!   and the `pressed` matcher.
//! - `validate` — schema validation for unknown-key detection.
//!
//! `load` merges the global config with project-local overrides, returning any
//! validation warning rather than failing the launch; `save_changes` writes
//! runtime changes back without destroying the source document.
//! Keep new fields in `types` in sync with the defaults so round-tripping
//! a saved config is lossless.

mod keymap;
pub mod static_keys;
mod types;
pub(crate) mod validate;

// Used by test helpers across modules to inject bindings.
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use keymap::bind;
pub use keymap::{pressed, pressed_in, BindingScope, Keymap};
#[allow(unused_imports)]
pub use types::UiConfig;
pub use types::{Config, GeneralConfig, StatusBarConfig};
// Only referenced in test struct literals.
#[cfg_attr(not(test), allow(unused_imports))]
pub use types::{TelemetryConfig, UpdatesConfig};
#[allow(unused_imports)]
pub use validate::schema_paths;
// Used by name only in test code (struct literals in *_test helpers).
#[cfg_attr(not(test), allow(unused_imports))]
pub use types::{GitConfig, GitDiffConfig, TreeConfig};
// Only referenced in doc links or via field access — never named in code.
#[allow(unused_imports)]
pub use keymap::KeyBinding;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
#[allow(unused_imports)]
pub use types::{ContentConfig, SearchConfig};

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
    let paths = config_paths(root);
    let mut error = None;
    let mut merged = toml::Value::Table(toml::map::Map::new());
    let mut loaded_path = None;
    let mut loaded_any = false;

    // Apply the least-specific file first so nearer project/ancestor files
    // override only the values they declare.
    for path in paths.iter().rev() {
        let Ok(s) = fs::read_to_string(path) else {
            continue; // missing or unreadable: try the next candidate
        };
        if let Err(e) = toml::from_str::<Config>(&s) {
            if error.is_none() {
                error = Some(format!("{}: {e}", path.display()));
            }
            continue;
        }
        let Ok(value) = toml::from_str::<toml::Value>(&s) else {
            continue;
        };
        if error.is_none() {
            let unknown = validate::validate_keys(&s);
            if !unknown.is_empty() {
                error = Some(format!("{}: {}", path.display(), unknown.join("; ")));
            }
        }
        merge_config_tables(&mut merged, value);
        loaded_any = true;
        loaded_path = Some(path.clone());
    }

    let mut config = if loaded_any {
        merged.try_into().unwrap_or_default()
    } else {
        Config::default()
    };
    config.migrate_legacy_flat_fields();
    config.migrate_legacy_git_fields();
    config.migrate_legacy_plugin_paths();
    config.keys.migrate_legacy_keys();
    let retired = crate::plugin::retired_bundled_plugins();
    config.plugins.retain(|_name, entry| {
        let Some(fname) = entry.path.file_name().and_then(|s| s.to_str()) else {
            return true;
        };
        !retired.contains(&fname)
    });
    (config, loaded_path.or(global), error)
}

/// Recursively merges a higher-precedence TOML table into `base`.
fn merge_config_tables(base: &mut toml::Value, overlay: toml::Value) {
    let (toml::Value::Table(base), toml::Value::Table(overlay)) = (base, overlay) else {
        return;
    };
    for (key, value) in overlay {
        if let Some(existing) = base.get_mut(&key) {
            if existing.is_table() && value.is_table() {
                merge_config_tables(existing, value);
                continue;
            }
        }
        base.insert(key, value);
    }
}

/// Persists the settings that changed between `previous` (the config as last
/// loaded or saved) and `current` into the file at `path`, editing it in place.
///
/// The file may be a hand-written, checked-in project `mantis.toml`, so it is
/// never regenerated: comments, key order, legacy spellings, and every key the
/// user did not change at runtime are preserved. Changed keys already present
/// in the file are replaced (keeping their trailing comments); new keys are
/// added only when they differ from the built-in defaults; removed keys are
/// deleted. Returns `Ok(false)` without touching the disk when nothing changed.
/// A missing file is created with [`sparse_toml`]; an unparseable one is left
/// alone and reported as an error rather than overwritten.
pub fn save_changes(previous: &Config, current: &Config, path: &Path) -> std::io::Result<bool> {
    let invalid = |e: String| std::io::Error::new(std::io::ErrorKind::InvalidData, e);
    let prev = toml::Value::try_from(previous).map_err(|e| invalid(e.to_string()))?;
    let cur = toml::Value::try_from(current).map_err(|e| invalid(e.to_string()))?;
    if prev == cur {
        return Ok(false);
    }
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            fs::write(path, sparse_toml(current))?;
            return Ok(true);
        }
        Err(e) => return Err(e),
    };
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(|e| {
        invalid(format!(
            "{} is not valid TOML, not overwriting it: {e}",
            path.display()
        ))
    })?;
    let default = toml::Value::try_from(Config::default()).ok();
    if let (toml::Value::Table(prev), toml::Value::Table(cur)) = (&prev, &cur) {
        let default = match &default {
            Some(toml::Value::Table(t)) => Some(t),
            _ => None,
        };
        apply_table_changes(doc.as_table_mut(), prev, cur, default);
    }
    fs::write(path, doc.to_string())?;
    Ok(true)
}

/// Applies the key-level differences between `prev` and `cur` to `table`,
/// recursing into sub-tables that exist on both sides and in the document.
fn apply_table_changes(
    table: &mut dyn toml_edit::TableLike,
    prev: &toml::map::Map<String, toml::Value>,
    cur: &toml::map::Map<String, toml::Value>,
    default: Option<&toml::map::Map<String, toml::Value>>,
) {
    let keys: std::collections::BTreeSet<&String> = prev.keys().chain(cur.keys()).collect();
    for key in keys {
        let before = prev.get(key);
        let Some(after) = cur.get(key) else {
            table.remove(key);
            continue;
        };
        if before == Some(after) {
            continue;
        }
        let key_default = default.and_then(|d| d.get(key));
        if let (Some(toml::Value::Table(before)), toml::Value::Table(after)) = (before, after) {
            let child_default = match key_default {
                Some(toml::Value::Table(t)) => Some(t),
                _ => None,
            };
            // A config section (one with defaults, e.g. `[ui]` or `[plugins]`)
            // missing from the file is created as an implicit table so only
            // its changed keys are written. Map entries without defaults
            // (a single `[plugins.<name>]`) fall through and are written whole.
            let created = !table.contains_key(key) && child_default.is_some() && !table.is_dotted();
            if created {
                let mut section = toml_edit::Table::new();
                section.set_implicit(true);
                table.insert(key, toml_edit::Item::Table(section));
            }
            if let Some(child) = table.get_mut(key).and_then(|i| i.as_table_like_mut()) {
                apply_table_changes(child, before, after, child_default);
                if created && child.is_empty() {
                    table.remove(key);
                }
                continue;
            }
        }
        let value = if table.contains_key(key) {
            after.clone()
        } else {
            match without_defaults(after, key_default) {
                Some(value) => value,
                None => continue,
            }
        };
        let Some(mut item) = toml_item(key, &value) else {
            continue;
        };
        // Replace an existing scalar/inline value in place: the comment lines
        // above a key belong to the key, so re-inserting would drop them.
        if let Some(existing) = table.get_mut(key).and_then(|i| i.as_value_mut()) {
            let decor = existing.decor().clone();
            if let Ok(mut new) = item.into_value() {
                *new.decor_mut() = decor;
                *existing = new;
            }
            continue;
        }
        if table.is_dotted() {
            item = match item.into_value() {
                Ok(value) => toml_edit::Item::Value(value),
                Err(item) => item,
            };
        }
        table.insert(key, item);
    }
}

/// `value` minus any parts equal to `default`, or `None` when nothing differs.
fn without_defaults(value: &toml::Value, default: Option<&toml::Value>) -> Option<toml::Value> {
    match (value, default) {
        (_, Some(default)) if default == value => None,
        (toml::Value::Table(value), Some(toml::Value::Table(default))) => {
            let kept: toml::map::Map<String, toml::Value> = value
                .iter()
                .filter_map(|(k, v)| Some((k.clone(), without_defaults(v, default.get(k))?)))
                .collect();
            (!kept.is_empty()).then_some(toml::Value::Table(kept))
        }
        _ => Some(value.clone()),
    }
}

/// Converts `value` into a `toml_edit` item formatted the way `toml` would
/// write it under `key` (tables as `[section]`s, scalars as plain values).
fn toml_item(key: &str, value: &toml::Value) -> Option<toml_edit::Item> {
    let mut wrapper = toml::map::Map::new();
    wrapper.insert(key.to_string(), value.clone());
    let text = toml::to_string(&toml::Value::Table(wrapper)).ok()?;
    let mut doc: toml_edit::DocumentMut = text.parse().ok()?;
    doc.as_table_mut().remove(key)
}

/// Serialises `config` keeping only the top-level keys whose value differs from
/// `Config::default()`. This keeps the user's `mantis.toml` a minimal override file:
/// untouched settings fall through to the embedded defaults rather than being
/// pinned to their current value (which would also mask future default changes).
///
/// For `[plugins]`, entries matching the default bundled definitions are omitted,
/// serialising only user-defined customizations (e.g. disabled bundled plugins or
/// custom plugin paths).
pub fn sparse_toml(config: &Config) -> String {
    let current = toml::Value::try_from(config);
    let default = toml::Value::try_from(Config::default());
    let (Ok(toml::Value::Table(cur)), Ok(toml::Value::Table(def))) = (current, default) else {
        // Serialisation should never fail for our own type; fall back to a full
        // dump rather than losing the user's settings.
        return toml::to_string_pretty(config).unwrap_or_default();
    };
    let bundled_defaults: HashMap<String, crate::plugin::PluginEntry> =
        crate::plugin::bundled_plugin_entries()
            .into_iter()
            .collect();
    let mut out = toml::map::Map::new();
    for (k, v) in &cur {
        if k == "plugins" {
            if let toml::Value::Table(ref plugins_map) = v {
                let mut sparse_plugins = toml::map::Map::new();
                for (pname, pval) in plugins_map {
                    let is_default = if let Some(bundled_entry) = bundled_defaults.get(pname) {
                        config
                            .plugins
                            .get(pname)
                            .map(|e| e == bundled_entry)
                            .unwrap_or(false)
                    } else {
                        false
                    };
                    if !is_default {
                        sparse_plugins.insert(pname.clone(), pval.clone());
                    }
                }
                if !sparse_plugins.is_empty() {
                    out.insert(k.clone(), toml::Value::Table(sparse_plugins));
                }
            }
        } else if def.get(k) != Some(v) {
            out.insert(k.clone(), v.clone());
        }
    }
    toml::to_string_pretty(&toml::Value::Table(out)).unwrap_or_default()
}

/// Prepares the global config directory on launch. The fully-commented default
/// reference (`mantis.default.toml`) and static keys reference (`mantis.static.toml`)
/// are refreshed whenever they are missing or stale (i.e. after an upgrade), so users
/// always have an up-to-date catalogue of every option and reserved key. The user's own
/// `mantis.toml` is **never** overwritten: it is created once, as a minimal stub, only
/// when absent. Bundled themes and plugins are seeded on that same first run.
fn init_config_dir(user_path: &Path) {
    let Some(dir) = user_path.parent() else {
        return;
    };
    let _ = fs::create_dir_all(dir);
    refresh_default_reference(dir);
    refresh_static_keys_reference(dir);
    if !user_path.exists() {
        let _ = fs::write(user_path, USER_CONFIG_STUB);
        crate::theme::install_embedded_themes();
    }
    // Always run: install/refresh bundled plugins and clean up retired ones.
    crate::plugin::install_bundled_plugins();
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

/// Writes the embedded static keys reference to `{dir}/mantis.static.toml`, but
/// only when the file is missing or stale. Returns whether the file was (re)written.
/// This is informational only; the static keys cannot be overridden.
fn refresh_static_keys_reference(dir: &Path) -> bool {
    let path = dir.join(STATIC_KEYS_REFERENCE_NAME);
    if fs::read_to_string(&path).ok().as_deref() == Some(STATIC_KEYS_TEMPLATE) {
        return false;
    }
    fs::write(&path, STATIC_KEYS_TEMPLATE).is_ok()
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

/// Reserved (non-configurable) modal keybindings reference.
/// This is informational only — these keys cannot be remapped via [keys].
const STATIC_KEYS_TEMPLATE: &str = "\
# ============================================================================
#  Reserved modal keybindings (non-configurable)
# ============================================================================
# This file documents the fixed keybindings for modal overlays and screens.
# These keys CANNOT be remapped via the [keys] section in mantis.toml.
# They are reserved to ensure consistent, predictable modal behavior.
#
# This is a read-only reference; changes here have no effect.
# ============================================================================

[modal_keys]
# Overlay navigation and activation
close = \"Esc\"                           # close/cancel any overlay
activate = \"Enter\"                      # activate selected item

# List navigation (search, history, picker, etc.)
list_up = [\"Up\", \"k\"]                 # move up
list_down = [\"Down\", \"j\"]             # move down
page_up = \"PageUp\"                      # scroll up by page
page_down = \"PageDown\"                  # scroll down by page

# Text input
delete_char = \"Backspace\"               # delete character or close if empty
input_char = \"<any printable>\"          # type to filter

# In-file search navigation
search_next = [\"Down\", \"n\", \"Tab\"]  # next match
search_prev = [\"Up\", \"N\", \"BackTab\", \"Ctrl+P\"]  # previous match

# About/Help screens
about_open_release = \"o\"                # open release URL (About screen)
modal_close = [\"?\", \"q\", \"Esc\", \"Enter\"]  # close modal (About/Help)

# Search overlay
toggle_search_mode = \"Tab\"              # toggle file/content search mode

# Plugin picker
plugin_toggle = \"Space\"                 # toggle plugin selection
";

/// Filename of the read-only static keys reference written next to the user config.
const STATIC_KEYS_REFERENCE_NAME: &str = "mantis.static.toml";

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
