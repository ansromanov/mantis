//! Themes shipped inside the binary and the user themes directory.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use super::Theme;

/// List of (name, toml_content) for each shipped theme.
pub(super) const EMBEDDED_MANIFEST: &[(&str, &str)] = &[
    ("default", include_str!("../../themes/default.toml")),
    ("monokai", include_str!("../../themes/monokai.toml")),
    ("solarized", include_str!("../../themes/solarized.toml")),
    ("catppuccin", include_str!("../../themes/catppuccin.toml")),
    ("synthwave84", include_str!("../../themes/synthwave84.toml")),
    (
        "vscode-light",
        include_str!("../../themes/vscode-light.toml"),
    ),
    (
        "catppuccin-latte",
        include_str!("../../themes/catppuccin-latte.toml"),
    ),
    (
        "solarized-light",
        include_str!("../../themes/solarized-light.toml"),
    ),
    ("pink", include_str!("../../themes/pink.toml")),
];

pub(super) fn all_embedded() -> &'static HashMap<&'static str, Theme> {
    static ALL_EMBEDDED: OnceLock<HashMap<&'static str, Theme>> = OnceLock::new();
    ALL_EMBEDDED.get_or_init(|| {
        let mut m = HashMap::new();
        for (name, toml) in EMBEDDED_MANIFEST {
            if let Some(theme) = Theme::from_toml(toml) {
                m.insert(*name, theme);
            }
        }
        m
    })
}

// ---------------------------------------------------------------------------
// User themes directory
// ---------------------------------------------------------------------------

pub(super) fn user_themes_dir() -> Option<PathBuf> {
    let is_test_env = cfg!(test)
        || std::env::current_exe()
            .map(|p| {
                p.parent()
                    .and_then(|parent| parent.file_name())
                    .map(|name| name == "deps")
                    .unwrap_or(false)
            })
            .unwrap_or(false);
    if is_test_env {
        #[cfg(windows)]
        let env_key = "APPDATA";
        #[cfg(not(windows))]
        let env_key = "XDG_CONFIG_HOME";

        let env_val = std::env::var_os(env_key);
        let is_isolated = env_val
            .as_ref()
            .map(|v| v.to_string_lossy().contains("tv2_theme_test"))
            .unwrap_or(false);

        if !is_isolated {
            return None;
        }
    }

    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("mantis").join("themes"))
    }
    #[cfg(not(windows))]
    {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("mantis").join("themes"))
    }
}

/// Copies every bundled theme to the user themes directory if it doesn't
/// already exist there, so users have local files to reference or edit.
pub fn install_embedded_themes() {
    let Some(dir) = user_themes_dir() else {
        return;
    };
    let _ = fs::create_dir_all(&dir);
    for (name, toml) in EMBEDDED_MANIFEST {
        let path = dir.join(format!("{name}.toml"));
        if !path.exists() {
            let _ = fs::write(&path, toml);
        }
    }
}

// ---------------------------------------------------------------------------
// ThemeConfig – user overrides from mantis.toml
// ---------------------------------------------------------------------------
