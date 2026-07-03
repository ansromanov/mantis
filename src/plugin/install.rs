//! Plugin installation and default directory discovery.
//!
//! Locates the platform-specific plugin directory, finds bundled binary
//! plugins next to the mantis binary or in cargo target directories, and
//! installs syntax definitions into `{plugin_dir}/syntaxes/`.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::plugin::types::{PluginEntry, PluginKind};

/// Returns `(name, PluginEntry)` pairs for every plugin that ships with `mantis`,
/// each pre-set to `enabled = false` so they appear in the palette without
/// being spawned automatically.
pub(crate) fn bundled_plugin_entries() -> Vec<(String, PluginEntry)> {
    let mut entries = Vec::new();
    for (name, binary_name) in BUNDLED_PLUGINS {
        let filename = if cfg!(windows) {
            format!("{binary_name}.exe")
        } else {
            binary_name.to_string()
        };
        // Store the path relative to `default_plugin_dir()` (resolved at spawn
        // time in `PluginManager::activate_all`). An absolute path here would be
        // serialised verbatim into the user's `mantis.toml` on the first plugin
        // toggle, pinning a machine-specific home directory into a config that is
        // meant to be portable.
        entries.push((
            name.to_string(),
            PluginEntry {
                path: PathBuf::from(filename),
                enabled: false,
                kind: PluginKind::Process,
                extensions: Vec::new(),
                syntax_file: None,
                events: Vec::new(),
            },
        ));
    }
    for (name, syntax_rel_path, extensions) in BUNDLED_SYNTAX_PLUGIN_ENTRIES {
        let extensions: Vec<String> = extensions.iter().map(|s| s.to_string()).collect();
        entries.push((
            name.to_string(),
            PluginEntry {
                path: PathBuf::from(syntax_rel_path),
                enabled: false,
                kind: PluginKind::Syntax,
                extensions,
                syntax_file: Some(PathBuf::from(syntax_rel_path)),
                events: Vec::new(),
            },
        ));
    }
    entries
}

/// Default plugin discovery directory.
///
/// - Linux/macOS: `$XDG_CONFIG_HOME/mantis/plugins/` (falls back to
///   `~/.config/mantis/plugins/` when the variable is unset)
/// - Windows:     `%APPDATA%\mantis\plugins\`
pub(crate) fn default_plugin_dir() -> PathBuf {
    dirs_next().unwrap_or_else(|| PathBuf::from("."))
}

fn dirs_next() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("mantis").join("plugins"))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|base| base.join("mantis").join("plugins"))
    }
}

/// Tries to find `cargo` on PATH. Returns `Some(path)` when found.
fn which_cargo() -> Option<String> {
    if let Ok(cargo) = std::env::var("CARGO") {
        return Some(cargo);
    }
    for dir in std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .unwrap_or_default()
    {
        let cand = dir.join(if cfg!(windows) { "cargo.exe" } else { "cargo" });
        if cand.is_file() {
            return Some(cand.to_string_lossy().into_owned());
        }
    }
    None
}

/// List of (user-facing_name, binary_name) for each bundled process plugin.
const BUNDLED_PLUGINS: &[(&str, &str)] = &[
    ("iconize", "mantis-plugin-iconize"),
    ("markdown", "mantis-plugin-markdown"),
];

/// Filenames of old shell-script plugins superseded by the current Rust binaries.
///
/// These were shipped by mantis up to 0.7.x and lived in the plugin directory
/// as standalone shell scripts. They are no longer needed and are silently
/// removed on startup so stale files cannot be accidentally re-enabled.
pub(crate) const RETIRED_BUNDLED_PLUGINS: &[&str] = &["git-diff.sh", "git-log.sh", "iconize.sh"];

/// List of (filename, content) for each bundled syntax definition.
const BUNDLED_SYNTAX_PLUGINS: &[(&str, &str)] = &[(
    "terraform.sublime-syntax",
    include_str!("../../plugins/terraform/syntaxes/terraform.sublime-syntax"),
)];

/// List of (name, syntax_rel_path, extensions) for syntax plugin [plugins] entries.
/// Seeded into the config so syntax plugins appear in the plugin palette.
const BUNDLED_SYNTAX_PLUGIN_ENTRIES: &[(&str, &str, &[&str])] = &[(
    "terraform",
    "syntaxes/terraform.sublime-syntax",
    &["tf", "tfvars"],
)];

/// Copies every bundled plugin to the plugin directory if it doesn't already
/// exist there. Syntax definitions go into `{plugin_dir}/syntaxes/`. Also
/// removes any retired shell-script plugins that were superseded by Rust
/// binaries (pre-0.8 upgrade cleanup).
pub(crate) fn install_bundled_plugins() {
    let dir = default_plugin_dir();
    let _ = std::fs::create_dir_all(&dir);

    for (_name, binary_name) in BUNDLED_PLUGINS {
        let binary_filename = if cfg!(windows) {
            format!("{binary_name}.exe")
        } else {
            binary_name.to_string()
        };
        let plugin_path = dir.join(&binary_filename);
        if plugin_path.exists() {
            continue;
        }
        install_one_binary(binary_name, &plugin_path);
    }

    let syntax_dir = dir.join("syntaxes");
    let _ = std::fs::create_dir_all(&syntax_dir);
    for (name, content) in BUNDLED_SYNTAX_PLUGINS {
        let path = syntax_dir.join(name);
        if !path.exists() {
            let _ = std::fs::write(&path, content);
        }
    }

    remove_retired_bundled_plugins();
}

/// Removes retired bundled plugin files from the plugin directory.
///
/// Deletes only exact-name matches that mantis is known to have shipped
/// (see `RETIRED_BUNDLED_PLUGINS`). User-authored plugins are never touched.
/// Errors are silently ignored so a permissions issue does not crash startup.
pub(crate) fn remove_retired_bundled_plugins() {
    let dir = default_plugin_dir();
    for name in RETIRED_BUNDLED_PLUGINS {
        let path = dir.join(name);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Returns the list of retired plugin filenames that should be removed
/// from the plugin directory and filtered out of config `[plugins]` entries.
pub(crate) fn retired_bundled_plugins() -> &'static [&'static str] {
    RETIRED_BUNDLED_PLUGINS
}

/// Searches for a compiled Rust binary and copies it to `dest`.
/// Tries alongside the mantis binary, then `target/debug/`, `target/release/`,
/// and finally builds from source in a background thread.
fn install_one_binary(binary_name: &str, dest: &Path) {
    let platform_name = if cfg!(windows) {
        format!("{binary_name}.exe")
    } else {
        binary_name.to_string()
    };

    let candidates: Vec<PathBuf> = {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));
        let mut c = Vec::new();
        if let Some(ref d) = exe_dir {
            c.push(d.join(&platform_name));
            c.push(d.join("..").join("debug").join(&platform_name));
            c.push(d.join("..").join("release").join(&platform_name));
        }
        c.push(PathBuf::from("target/debug").join(&platform_name));
        c.push(PathBuf::from("target/release").join(&platform_name));
        c
    };

    for cand in &candidates {
        if cand.exists() {
            if std::fs::copy(cand, dest).is_ok() {
                set_executable(dest);
            }
            return;
        }
    }

    if let Some(cargo) = which_cargo() {
        if PathBuf::from("Cargo.toml").exists() {
            let dest = dest.to_path_buf();
            let pkg_name = binary_name.to_string();
            let platform_name_clone = platform_name.clone();
            std::thread::spawn(move || {
                let status = Command::new(&cargo)
                    .arg("build")
                    .arg("--package")
                    .arg(&pkg_name)
                    .arg("--release")
                    .status();
                if status.map(|s| s.success()).unwrap_or(false) {
                    let release_path = PathBuf::from("target/release").join(&platform_name_clone);
                    if release_path.exists() {
                        let _ = std::fs::copy(&release_path, &dest);
                        set_executable(&dest);
                    }
                }
            });
        }
    }
}

#[cfg(unix)]
fn set_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755));
}
#[cfg(not(unix))]
fn set_executable(_path: &Path) {}
