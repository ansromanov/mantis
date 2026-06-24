use std::path::PathBuf;

use super::*;
use crate::plugin::syntax::{collect_syntax_plugins, discover_syntax_plugins};

#[test]
fn collect_syntax_plugins_filters_by_kind_and_enabled() {
    let entries: Vec<(String, PluginEntry)> = vec![
        (
            "proc-p".into(),
            PluginEntry {
                path: "/bin/foo".into(),
                enabled: true,
                kind: PluginKind::Process,
                ..Default::default()
            },
        ),
        (
            "syn-p".into(),
            PluginEntry {
                kind: PluginKind::Syntax,
                enabled: true,
                syntax_file: Some("my-syntax.sublime-syntax".into()),
                ..Default::default()
            },
        ),
        (
            "disabled-syn".into(),
            PluginEntry {
                kind: PluginKind::Syntax,
                enabled: false,
                syntax_file: Some("disabled.sublime-syntax".into()),
                ..Default::default()
            },
        ),
    ];
    let result = collect_syntax_plugins(&entries);
    assert_eq!(result.len(), 1, "only the enabled syntax plugin");
    assert!(result[0]
        .syntax_path
        .to_string_lossy()
        .ends_with("my-syntax.sublime-syntax"));
}

#[test]
fn collect_syntax_plugins_skips_entries_without_syntax_file() {
    let entries: Vec<(String, PluginEntry)> = vec![(
        "no-file".into(),
        PluginEntry {
            kind: PluginKind::Syntax,
            enabled: true,
            syntax_file: None,
            ..Default::default()
        },
    )];
    assert!(collect_syntax_plugins(&entries).is_empty());
}

#[test]
fn discover_syntax_plugins_finds_sublime_files() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = std::env::temp_dir().join(format!("tv_syntax_discover_{}", std::process::id()));
    let old = std::env::var_os("XDG_CONFIG_HOME");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &tmp) };

    let syntax_dir = default_plugin_dir().join("syntaxes");
    std::fs::create_dir_all(&syntax_dir).unwrap();
    std::fs::write(syntax_dir.join("terraform.sublime-syntax"), "content").unwrap();
    std::fs::write(syntax_dir.join("readme.txt"), "not a syntax file").unwrap();

    let result = discover_syntax_plugins(&[]);

    unsafe {
        match old {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }

    assert_eq!(result.len(), 1, "only .sublime-syntax files");
    assert!(result[0]
        .syntax_path
        .to_string_lossy()
        .ends_with("terraform.sublime-syntax"));
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn discover_syntax_plugins_skips_managed_files() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = std::env::temp_dir().join(format!("tv_syntax_managed_{}", std::process::id()));
    let old = std::env::var_os("XDG_CONFIG_HOME");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &tmp) };

    let syntax_dir = default_plugin_dir().join("syntaxes");
    std::fs::create_dir_all(&syntax_dir).unwrap();
    std::fs::write(syntax_dir.join("terraform.sublime-syntax"), "content").unwrap();
    std::fs::write(syntax_dir.join("other.sublime-syntax"), "content2").unwrap();

    // A config entry manages terraform.sublime-syntax
    let entries: Vec<(String, PluginEntry)> = vec![(
        "terraform".into(),
        PluginEntry {
            kind: PluginKind::Syntax,
            enabled: false,
            syntax_file: Some(PathBuf::from("syntaxes/terraform.sublime-syntax")),
            ..Default::default()
        },
    )];

    let result = discover_syntax_plugins(&entries);

    unsafe {
        match old {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }

    assert_eq!(
        result.len(),
        1,
        "only the unmanaged syntax file should be discovered"
    );
    assert!(
        result[0]
            .syntax_path
            .to_string_lossy()
            .ends_with("other.sublime-syntax"),
        "the managed terraform file must be skipped"
    );
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn discover_syntax_plugins_returns_empty_when_no_syntaxes_dir() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = std::env::temp_dir().join(format!("tv_no_syntaxes_{}", std::process::id()));
    let old = std::env::var_os("XDG_CONFIG_HOME");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &tmp) };

    let result = discover_syntax_plugins(&[]);

    unsafe {
        match old {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }

    assert!(result.is_empty());
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn load_extra_syntaxes_deduplicates_by_path() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = std::env::temp_dir().join(format!("tv_syntax_load_{}", std::process::id()));
    let old = std::env::var_os("XDG_CONFIG_HOME");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &tmp) };

    let syntax_dir = default_plugin_dir().join("syntaxes");
    std::fs::create_dir_all(&syntax_dir).unwrap();
    let shared_path = syntax_dir.join("shared.sublime-syntax");
    std::fs::write(&shared_path, "content").unwrap();

    // Same path referenced from both a config entry and auto-discovery
    let entries: Vec<(String, PluginEntry)> = vec![(
        "explicit".into(),
        PluginEntry {
            kind: PluginKind::Syntax,
            enabled: true,
            syntax_file: Some(shared_path.clone()),
            ..Default::default()
        },
    )];

    let result = load_extra_syntaxes(&entries);

    unsafe {
        match old {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }

    assert_eq!(result.len(), 1, "duplicate path deduplicated");
    std::fs::remove_dir_all(&tmp).ok();
}
