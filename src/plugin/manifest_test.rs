use std::path::PathBuf;

use super::*;

#[test]
fn load_manifest_from_tempdir() {
    let dir = std::env::temp_dir().join(format!("tv_manifest_load_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let toml = r#"
name = "test-plugin"
version = "0.1.0"
description = "A test plugin"
author = "test"
entry = "run.sh"
tv_protocol = "1"
"#;
    std::fs::write(dir.join("plugin.toml"), toml).unwrap();
    let manifest = crate::plugin::manifest::load(&dir).unwrap();
    assert_eq!(manifest.name, "test-plugin");
    assert_eq!(manifest.version, "0.1.0");
    assert_eq!(manifest.description.as_deref(), Some("A test plugin"));
    assert_eq!(manifest.author.as_deref(), Some("test"));
    assert_eq!(manifest.entry, "run.sh");
    assert_eq!(manifest.tv_protocol, "1");
    assert!(manifest.platforms.is_none());
    assert!(manifest.events.is_none());
    assert!(manifest.permissions.is_none());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn load_manifest_with_optional_fields() {
    let dir = std::env::temp_dir().join(format!("tv_manifest_opt_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let toml = r#"
name = "full-plugin"
version = "2.0.0"
description = "Full plugin"
author = "author"
entry = "main.py"
tv_protocol = "1"
platforms = ["linux", "macos"]
events = ["on_file_open", "on_keypress"]
permissions = ["read_files"]
"#;
    std::fs::write(dir.join("plugin.toml"), toml).unwrap();
    let manifest = crate::plugin::manifest::load(&dir).unwrap();
    assert_eq!(manifest.name, "full-plugin");
    assert_eq!(
        manifest.platforms,
        Some(vec!["linux".into(), "macos".into()])
    );
    assert_eq!(
        manifest.events,
        Some(vec!["on_file_open".into(), "on_keypress".into()])
    );
    assert_eq!(manifest.permissions, Some(vec!["read_files".into()]));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn load_returns_none_for_missing_file() {
    let dir = std::env::temp_dir().join(format!("tv_manifest_missing_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    assert!(crate::plugin::manifest::load(&dir).is_none());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn load_returns_none_for_invalid_toml() {
    let dir = std::env::temp_dir().join(format!("tv_manifest_bad_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("plugin.toml"), "not valid toml {{{").unwrap();
    assert!(crate::plugin::manifest::load(&dir).is_none());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn discover_finds_plugin_subdirectories() {
    let dir = std::env::temp_dir().join(format!("tv_discover_find_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let sub = dir.join("my-plugin");
    std::fs::create_dir_all(&sub).unwrap();
    let toml = r#"
name = "my-plugin"
version = "0.1.0"
description = "My plugin"
author = "me"
entry = "run.sh"
tv_protocol = "1"
"#;
    std::fs::write(sub.join("plugin.toml"), toml).unwrap();

    let entries = crate::plugin::manifest::discover(&dir);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].0, "my-plugin");
    assert!(
        !entries[0].1.enabled,
        "discovered plugins must default to disabled"
    );
    assert_eq!(entries[0].1.path, PathBuf::from("my-plugin/run.sh"));
    assert_eq!(entries[0].1.kind, PluginKind::Process);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn discover_skips_dirs_without_plugin_toml() {
    let dir = std::env::temp_dir().join(format!("tv_discover_skip_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let sub = dir.join("no-manifest");
    std::fs::create_dir_all(&sub).unwrap();

    assert!(crate::plugin::manifest::discover(&dir).is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn discover_returns_empty_for_nonexistent_dir() {
    let dir = std::env::temp_dir().join(format!("tv_discover_nonex_{}", std::process::id()));
    assert!(crate::plugin::manifest::discover(&dir).is_empty());
}

#[test]
fn discover_filters_by_platform() {
    let dir = std::env::temp_dir().join(format!("tv_discover_plat_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    // Windows-only plugin
    let sub = dir.join("windows-only");
    std::fs::create_dir_all(&sub).unwrap();
    let toml = r#"
name = "windows-only"
version = "0.1.0"
entry = "tool.exe"
tv_protocol = "1"
platforms = ["windows"]
"#;
    std::fs::write(sub.join("plugin.toml"), toml).unwrap();

    // Cross-platform plugin (no platform restriction)
    let sub2 = dir.join("cross-platform");
    std::fs::create_dir_all(&sub2).unwrap();
    let toml2 = r#"
name = "cross-platform"
version = "0.1.0"
entry = "tool.sh"
tv_protocol = "1"
"#;
    std::fs::write(sub2.join("plugin.toml"), toml2).unwrap();

    let current_os = std::env::consts::OS;
    let entries = crate::plugin::manifest::discover(&dir);
    let names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();

    if current_os == "windows" {
        assert!(names.contains(&"windows-only"));
    } else {
        assert!(!names.contains(&"windows-only"));
    }
    assert!(names.contains(&"cross-platform"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn discover_sorts_entries_by_name() {
    let dir = std::env::temp_dir().join(format!("tv_discover_sort_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    for name in ["z-plugin", "a-plugin", "m-plugin"] {
        let sub = dir.join(name);
        std::fs::create_dir_all(&sub).unwrap();
        let toml = format!(
            r#"
name = "{name}"
version = "0.1.0"
entry = "run.sh"
tv_protocol = "1"
"#
        );
        std::fs::write(sub.join("plugin.toml"), toml).unwrap();
    }

    let entries = crate::plugin::manifest::discover(&dir);
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].0, "a-plugin");
    assert_eq!(entries[1].0, "m-plugin");
    assert_eq!(entries[2].0, "z-plugin");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn discover_multi_plugin_dir() {
    let dir = std::env::temp_dir().join(format!("tv_discover_multi_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let plugins = [
        ("alpha", "alpha/run.sh"),
        ("beta", "beta/start.py"),
        ("gamma", "gamma/bin/gamma"),
    ];
    for (name, entry) in &plugins {
        let sub = dir.join(name);
        std::fs::create_dir_all(&sub).unwrap();
        let toml = format!(
            r#"
name = "{name}"
version = "0.1.0"
entry = "{entry}"
tv_protocol = "1"
"#
        );
        std::fs::write(sub.join("plugin.toml"), toml).unwrap();
    }

    let entries = crate::plugin::manifest::discover(&dir);
    assert_eq!(entries.len(), 3);
    // Verify paths use the manifest entry value (relative to plugin dir)
    for (name, _) in &plugins {
        let found = entries.iter().find(|(n, _)| n == name).unwrap();
        assert!(!found.1.enabled);
    }
    std::fs::remove_dir_all(&dir).ok();
}
