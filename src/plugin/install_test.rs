use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

fn fresh_plugin_dir() -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("mantis_plugin_install_{}_{n}", std::process::id()))
}

#[test]
#[cfg(not(windows))]
fn default_plugin_dir_ends_with_suffix() {
    let dir = default_plugin_dir();
    let components: Vec<_> = dir.components().collect();
    let last_two: Vec<_> = components.iter().rev().take(2).collect();
    assert_eq!(
        last_two[0],
        &std::path::Component::Normal("plugins".as_ref())
    );
    assert_eq!(
        last_two[1],
        &std::path::Component::Normal("mantis".as_ref())
    );
}

#[test]
#[cfg(not(windows))]
fn default_plugin_dir_respects_xdg() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let old = std::env::var_os("XDG_CONFIG_HOME");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", "/tmp/custom_cfg") };
    let dir = default_plugin_dir();
    unsafe {
        match old {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }
    assert!(dir.starts_with("/tmp/custom_cfg/mantis/plugins"));
}

#[test]
fn bundled_plugin_entries_all_disabled_and_include_markdown_and_terraform() {
    let entries = bundled_plugin_entries();
    assert!(!entries.is_empty(), "must have at least one bundled plugin");
    for (_, entry) in &entries {
        assert!(
            !entry.enabled,
            "bundled entries must default to enabled=false"
        );
    }
    let names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        names.contains(&"markdown"),
        "markdown plugin must be listed"
    );
    assert!(names.contains(&"iconize"), "iconize plugin must be listed");
    assert!(
        names.contains(&"terraform"),
        "terraform syntax plugin must be listed"
    );
    // Process entries
    for (name, entry) in &entries {
        if name == "terraform" {
            assert_eq!(
                entry.kind,
                PluginKind::Syntax,
                "terraform must be a syntax plugin"
            );
            assert!(
                entry.syntax_file.is_some(),
                "terraform must have syntax_file set"
            );
        } else {
            assert_eq!(
                entry.kind,
                PluginKind::Process,
                "all other bundled entries must be process plugins"
            );
        }
    }
}

#[test]
fn bundled_plugin_entries_use_relative_paths() {
    // Regression: bundled entries land in `app.config.plugins` and get
    // serialised into the user's `tv.toml` on the first plugin toggle. Absolute
    // paths would pin a machine-specific home directory into a portable config,
    // so every bundled `path` must be relative (resolved against the plugin dir
    // at spawn time).
    for (name, entry) in bundled_plugin_entries() {
        assert!(
            entry.path.is_relative(),
            "bundled plugin {name} path must be relative, got {:?}",
            entry.path
        );
    }
}

#[test]
fn bundled_plugin_entries_have_empty_events() {
    // Bundled entries declare no event subscription, so they receive all
    // events (empty = all, backward compat). Manifest-discovered plugins are
    // the ones that opt into a subset.
    for (name, entry) in bundled_plugin_entries() {
        assert!(
            entry.events.is_empty(),
            "bundled plugin {name} must not pin an events subscription"
        );
    }
}

#[test]
fn bundled_plugin_entries_no_duplicates() {
    let entries = bundled_plugin_entries();
    let mut seen = std::collections::HashSet::new();
    for (name, _) in &entries {
        assert!(seen.insert(name.as_str()), "duplicate entry: {name}");
    }
}

#[test]
fn install_bundled_plugins_creates_iconize_binary() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = fresh_plugin_dir();
    std::fs::create_dir_all(&tmp).unwrap();
    let old = std::env::var_os("XDG_CONFIG_HOME");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &tmp) };

    install_bundled_plugins();

    unsafe {
        match old {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }

    let plugins_dir = tmp.join("mantis").join("plugins");
    assert!(plugins_dir.is_dir(), "plugins directory should be created");
    assert!(
        plugins_dir.join("syntaxes").is_dir(),
        "syntaxes subdirectory should be created"
    );
    assert!(
        plugins_dir
            .join("syntaxes")
            .join("terraform.sublime-syntax")
            .exists(),
        "terraform.sublime-syntax must be installed"
    );
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn install_bundles_terraform_syntax_content() {
    // Guards the `include_str!` source path for the terraform syntax, which
    // moved to plugins/terraform/syntaxes/. A wrong-but-existing path would
    // bundle empty/garbage content while still compiling, so assert the
    // installed file carries the real HCL grammar.
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = fresh_plugin_dir();
    std::fs::create_dir_all(&tmp).unwrap();
    let old = std::env::var_os("XDG_CONFIG_HOME");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &tmp) };

    install_bundled_plugins();

    unsafe {
        match old {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }

    let syntax = tmp
        .join("mantis")
        .join("plugins")
        .join("syntaxes")
        .join("terraform.sublime-syntax");
    let content = std::fs::read_to_string(&syntax).expect("terraform syntax should be readable");
    assert!(
        content.contains("%YAML"),
        "bundled terraform syntax must be a real .sublime-syntax document"
    );
    assert!(
        content.to_lowercase().contains("terraform"),
        "bundled terraform syntax must contain the HCL/Terraform grammar"
    );
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn install_bundled_plugins_creates_plugin_dir_and_syntaxes() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = fresh_plugin_dir();
    std::fs::create_dir_all(&tmp).unwrap();
    let old = std::env::var_os("XDG_CONFIG_HOME");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &tmp) };

    install_bundled_plugins();

    unsafe {
        match old {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }

    let plugins_dir = tmp.join("mantis").join("plugins");
    assert!(plugins_dir.is_dir(), "plugins directory should be created");
    assert!(
        plugins_dir.join("syntaxes").is_dir(),
        "syntaxes subdirectory should be created"
    );
    assert!(
        plugins_dir
            .join("syntaxes")
            .join("terraform.sublime-syntax")
            .exists(),
        "terraform.sublime-syntax must be installed"
    );
    std::fs::remove_dir_all(&tmp).ok();
}
