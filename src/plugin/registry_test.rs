use super::*;

fn sample_index() -> RegistryIndex {
    RegistryIndex {
        plugins: vec![
            RegistryEntry {
                name: "git-tools".into(),
                description: "git diff/log integration for tv".into(),
                repo: "https://github.com/example/tv-git-tools".into(),
                tag: "v0.1.0".into(),
            },
            RegistryEntry {
                name: "markdown-preview".into(),
                description: "Live markdown preview panel".into(),
                repo: "https://github.com/example/tv-md-preview".into(),
                tag: "v0.2.0".into(),
            },
            RegistryEntry {
                name: "hex-viewer".into(),
                description: "hexadecimal file viewer".into(),
                repo: "https://github.com/example/tv-hex".into(),
                tag: "v0.1.1".into(),
            },
        ],
    }
}

// -- load_index / JSON parse ------------------------------------------------

#[test]
fn parse_valid_index() {
    let json = r#"{
        "plugins": [
            { "name": "a", "description": "desc a", "repo": "https://r.com/a", "tag": "v1" },
            { "name": "b", "description": "desc b", "repo": "https://r.com/b", "tag": "v2" }
        ]
    }"#;
    let index: RegistryIndex = serde_json::from_str(json).unwrap();
    assert_eq!(index.plugins.len(), 2);
    assert_eq!(index.plugins[0].name, "a");
    assert_eq!(index.plugins[1].tag, "v2");
}

#[test]
fn parse_empty_plugins_array() {
    let json = r#"{"plugins": []}"#;
    let index: RegistryIndex = serde_json::from_str(json).unwrap();
    assert!(index.plugins.is_empty());
}

#[test]
fn parse_minimal_entry() {
    let json = r#"{
        "plugins": [
            { "name": "x", "description": "", "repo": "https://x.com/x", "tag": "latest" }
        ]
    }"#;
    let index: RegistryIndex = serde_json::from_str(json).unwrap();
    assert_eq!(index.plugins.len(), 1);
    assert_eq!(index.plugins[0].name, "x");
    assert_eq!(index.plugins[0].description, "");
}

#[test]
fn parse_invalid_json_returns_error() {
    let result: Result<RegistryIndex, _> = serde_json::from_str("not json");
    assert!(result.is_err());
}

#[test]
fn parse_missing_field_returns_error() {
    let json = r#"{"plugins": [{"name": "x", "repo": "https://x.com/x"}]}"#;
    let result: Result<RegistryIndex, _> = serde_json::from_str(json);
    assert!(result.is_err(), "missing 'tag' must fail");
}

// -- search -----------------------------------------------------------------

#[test]
fn search_empty_query_returns_all() {
    let index = sample_index();
    let results = search(&index, "");
    assert_eq!(results.len(), 3);
}

#[test]
fn search_matches_name() {
    let index = sample_index();
    let results = search(&index, "git");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "git-tools");
}

#[test]
fn search_matches_description() {
    let index = sample_index();
    let results = search(&index, "hexadecimal");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "hex-viewer");
}

#[test]
fn search_case_insensitive() {
    let index = sample_index();
    let results = search(&index, "GIT");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "git-tools");
}

#[test]
fn search_matches_multiple() {
    let index = sample_index();
    let results = search(&index, "git");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "git-tools");
}

#[test]
fn search_matches_name_and_description() {
    let index = sample_index();
    let results = search(&index, "view");
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].name, "hex-viewer");
    assert_eq!(results[1].name, "markdown-preview");
}

#[test]
fn search_no_matches() {
    let index = sample_index();
    let results = search(&index, "zzzzz");
    assert!(results.is_empty());
}

#[test]
fn search_results_sorted_by_name() {
    let index = RegistryIndex {
        plugins: vec![
            RegistryEntry {
                name: "zebra".into(),
                description: "z".into(),
                repo: "https://r.com/z".into(),
                tag: "v1".into(),
            },
            RegistryEntry {
                name: "alpha".into(),
                description: "a".into(),
                repo: "https://r.com/a".into(),
                tag: "v1".into(),
            },
        ],
    };
    let results = search(&index, "");
    assert_eq!(results[0].name, "alpha");
    assert_eq!(results[1].name, "zebra");
}

// -- resolve ----------------------------------------------------------------

#[test]
fn resolve_exact_name_finds_entry() {
    let index = sample_index();
    let entry = resolve(&index, "git-tools");
    assert!(entry.is_some());
    assert_eq!(
        entry.unwrap().repo,
        "https://github.com/example/tv-git-tools"
    );
}

#[test]
fn resolve_case_sensitive() {
    let index = sample_index();
    assert!(resolve(&index, "Git-Tools").is_none());
    assert!(resolve(&index, "git-tools").is_some());
}

#[test]
fn resolve_unknown_name_returns_none() {
    let index = sample_index();
    assert!(resolve(&index, "nonexistent").is_none());
}

#[test]
fn resolve_empty_index_returns_none() {
    let index = RegistryIndex::default();
    assert!(resolve(&index, "anything").is_none());
}

// -- registry_dir -----------------------------------------------------------

#[test]
fn registry_dir_respects_env_override() {
    let _guard = crate::plugin::ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let old = std::env::var_os("MANTIS_PLUGIN_REGISTRY_DIR");
    // SAFETY: ENV_LOCK serialises all callers; no other thread mutates this var.
    unsafe { std::env::set_var("MANTIS_PLUGIN_REGISTRY_DIR", "/tmp/custom-registry") };
    let dir = registry_dir();
    unsafe {
        match old {
            Some(v) => std::env::set_var("MANTIS_PLUGIN_REGISTRY_DIR", v),
            None => std::env::remove_var("MANTIS_PLUGIN_REGISTRY_DIR"),
        }
    }
    assert_eq!(dir, PathBuf::from("/tmp/custom-registry"));
}

// -- clone_or_pull integration ----------------------------------------------
// These tests verify the shell-out logic using temp directories.

#[test]
fn clone_or_pull_initializes_bare_registry() {
    let _guard = crate::plugin::ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    // Create a bare registry repo to serve as the remote.
    let remote = std::env::temp_dir().join(format!("mantis_reg_remote_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&remote);
    let status = Command::new("git")
        .args(["init", "-q", "--bare"])
        .arg(&remote)
        .status()
        .expect("git init --bare");
    assert!(status.success());

    // Clone the bare repo, create index.json, push.
    let work = std::env::temp_dir().join(format!("mantis_reg_work_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    let status = Command::new("git")
        .args(["clone", "-q"])
        .arg(&remote)
        .arg(&work)
        .status()
        .expect("git clone");
    assert!(status.success());

    std::fs::write(
        work.join("index.json"),
        r#"{"plugins":[{"name":"test-plug","description":"test plugin","repo":"https://r.com/tp","tag":"v1"}]}"#,
    )
    .unwrap();

    let status = Command::new("git")
        .arg("-C")
        .arg(&work)
        .args(["add", "index.json"])
        .status()
        .unwrap();
    assert!(status.success());
    let status = Command::new("git")
        .arg("-C")
        .arg(&work)
        .args([
            "-c",
            "user.email=t@t.com",
            "-c",
            "user.name=T",
            "commit",
            "-q",
            "-m",
            "init",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let status = Command::new("git")
        .arg("-C")
        .arg(&work)
        .args(["push", "-q"])
        .status()
        .unwrap();
    assert!(status.success());

    // Point the registry at our remote.
    let old_repo = std::env::var_os("MANTIS_PLUGIN_REGISTRY");
    unsafe { std::env::set_var("MANTIS_PLUGIN_REGISTRY", remote.to_str().unwrap()) };

    let old_dir = std::env::var_os("MANTIS_PLUGIN_REGISTRY_DIR");
    let cache = std::env::temp_dir().join(format!("mantis_reg_cache_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cache);
    unsafe { std::env::set_var("MANTIS_PLUGIN_REGISTRY_DIR", cache.to_str().unwrap()) };

    let result = clone_or_pull();
    assert!(result.is_ok(), "clone should succeed: {:?}", result.err());

    // Verify the index.json was cloned.
    assert!(cache.join("index.json").exists());
    let index = load_index();
    assert!(index.is_some());
    assert_eq!(index.as_ref().unwrap().plugins.len(), 1);
    assert_eq!(index.unwrap().plugins[0].name, "test-plug");

    // Second call: pull should also succeed.
    let result = clone_or_pull();
    assert!(result.is_ok(), "pull should succeed: {:?}", result.err());

    // Cleanup.
    let _ = std::fs::remove_dir_all(&remote);
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&cache);
    unsafe {
        match old_repo {
            Some(v) => std::env::set_var("MANTIS_PLUGIN_REGISTRY", v),
            None => std::env::remove_var("MANTIS_PLUGIN_REGISTRY"),
        }
        match old_dir {
            Some(v) => std::env::set_var("MANTIS_PLUGIN_REGISTRY_DIR", v),
            None => std::env::remove_var("MANTIS_PLUGIN_REGISTRY_DIR"),
        }
    }
}

#[test]
fn clone_or_pull_with_invalid_url_returns_error() {
    let _guard = crate::plugin::ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let old_dir = std::env::var_os("MANTIS_PLUGIN_REGISTRY_DIR");
    let cache = std::env::temp_dir().join(format!("mantis_reg_bad_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cache);
    unsafe { std::env::set_var("MANTIS_PLUGIN_REGISTRY_DIR", cache.to_str().unwrap()) };
    let old_repo = std::env::var_os("MANTIS_PLUGIN_REGISTRY");
    unsafe {
        std::env::set_var(
            "MANTIS_PLUGIN_REGISTRY",
            "https://not-a-real-registry.local/test",
        )
    };

    let result = clone_or_pull();
    assert!(result.is_err());

    let _ = std::fs::remove_dir_all(&cache);
    unsafe {
        match old_repo {
            Some(v) => std::env::set_var("MANTIS_PLUGIN_REGISTRY", v),
            None => std::env::remove_var("MANTIS_PLUGIN_REGISTRY"),
        }
        match old_dir {
            Some(v) => std::env::set_var("MANTIS_PLUGIN_REGISTRY_DIR", v),
            None => std::env::remove_var("MANTIS_PLUGIN_REGISTRY_DIR"),
        }
    }
}
