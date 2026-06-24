use std::path::PathBuf;

use super::*;

fn make_reg(name: &str, exts: &[&str], caps: &[Capability]) -> LanguageProviderRegistration {
    LanguageProviderRegistration {
        plugin_name: name.to_string(),
        extensions: exts.iter().map(|e| e.to_string()).collect(),
        capabilities: caps.iter().cloned().collect(),
    }
}

#[test]
fn plugin_entries_empty_when_no_plugins_registered() {
    let mgr = PluginManager::new(vec![]);
    assert!(mgr.plugin_entries().is_empty());
}

#[test]
fn plugin_entries_shows_registered_plugins_as_not_running() {
    let entry = PluginEntry {
        path: PathBuf::from("/nonexistent/plugin"),
        enabled: false,
        ..Default::default()
    };
    let mgr = PluginManager::new(vec![("test-plugin".to_string(), entry)]);
    let entries = mgr.plugin_entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].0, "test-plugin");
    assert!(!entries[0].1, "unstarted plugin must not show as running");
}

#[test]
fn plugin_entries_reports_syntax_active_from_enabled_flag() {
    let entry = PluginEntry {
        kind: PluginKind::Syntax,
        enabled: false,
        ..Default::default()
    };
    let mut mgr = PluginManager::new(vec![("terraform".to_string(), entry)]);

    // Syntax plugins never spawn a subprocess; the checkbox follows `enabled`.
    assert!(
        !mgr.plugin_entries()[0].1,
        "disabled syntax plugin must show as off"
    );

    mgr.set_enabled("terraform", true);
    assert!(
        mgr.plugin_entries()[0].1,
        "enabled syntax plugin must show as on"
    );

    mgr.set_enabled("terraform", false);
    assert!(
        !mgr.plugin_entries()[0].1,
        "re-disabled syntax plugin must show as off"
    );
}

#[test]
fn activate_one_errors_on_unknown_name() {
    let mut mgr = PluginManager::new(vec![]);
    assert!(mgr.activate_one("ghost", None).is_err());
}

#[test]
fn activate_one_errors_on_bad_path() {
    let entry = PluginEntry {
        path: PathBuf::from("/nonexistent/plugin"),
        enabled: false,
        ..Default::default()
    };
    let mut mgr = PluginManager::new(vec![("bad".to_string(), entry)]);
    assert!(mgr.activate_one("bad", None).is_err());
}

#[test]
fn deactivate_one_is_noop_when_plugin_not_running() {
    let entry = PluginEntry {
        path: PathBuf::from("/nonexistent/plugin"),
        enabled: false,
        ..Default::default()
    };
    let mut mgr = PluginManager::new(vec![("p".to_string(), entry)]);
    mgr.deactivate_one("p");
    assert!(!mgr.plugin_entries()[0].1);
}

#[test]
#[cfg(unix)]
fn activate_one_then_deactivate_one_updates_running_state() {
    let entry = PluginEntry {
        path: PathBuf::from("/bin/cat"),
        enabled: false,
        ..Default::default()
    };
    let mut mgr = PluginManager::new(vec![("cat-stub".to_string(), entry)]);

    assert!(
        !mgr.plugin_entries()[0].1,
        "should not be running before activate"
    );
    mgr.activate_one("cat-stub", None).expect("spawn /bin/cat");
    assert!(
        mgr.plugin_entries()[0].1,
        "should be running after activate"
    );
    mgr.deactivate_one("cat-stub");
    assert!(
        !mgr.plugin_entries()[0].1,
        "should not be running after deactivate"
    );
}

#[test]
#[cfg(unix)]
fn activate_one_is_noop_when_already_running() {
    let entry = PluginEntry {
        path: PathBuf::from("/bin/cat"),
        enabled: false,
        ..Default::default()
    };
    let mut mgr = PluginManager::new(vec![("cat-stub".to_string(), entry)]);
    mgr.activate_one("cat-stub", None).expect("first spawn");
    mgr.activate_one("cat-stub", None)
        .expect("second call must be noop");
    assert_eq!(
        mgr.plugin_entries().iter().filter(|(_, r, _)| *r).count(),
        1,
        "must still be only one running instance"
    );
    mgr.deactivate_all();
}

#[test]
#[cfg(unix)]
fn activate_one_sends_init_with_protocol_version() {
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    let dir = std::env::temp_dir().join(format!("tv_mgr_init_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("recv.txt");

    // Recording stub: copies everything on stdin to a file, exits on EOF.
    let script = dir.join("rec.sh");
    let mut f = std::fs::File::create(&script).unwrap();
    write!(f, "#!/bin/sh\ncat > \"{}\"\n", out.display()).unwrap();
    drop(f);
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let entry = PluginEntry {
        path: script.clone(),
        enabled: false,
        ..Default::default()
    };
    let mut mgr = PluginManager::new(vec![("rec".to_string(), entry)]);
    mgr.activate_one("rec", None).expect("spawn rec.sh");
    // Closing stdin lets the stub flush and exit.
    mgr.deactivate_one("rec");

    let deadline = Instant::now() + Duration::from_secs(3);
    let contents = loop {
        if let Ok(s) = std::fs::read_to_string(&out) {
            if !s.is_empty() {
                break s;
            }
        }
        assert!(Instant::now() < deadline, "plugin never received init");
        std::thread::sleep(Duration::from_millis(25));
    };
    let init_line = contents
        .lines()
        .find(|l| l.contains(r#""event":"init""#))
        .expect("init event must be sent");
    assert!(
        init_line.contains(r#""protocol_version":"2""#),
        "init must carry host protocol version, got: {init_line}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn register_provider_and_provider_for_found() {
    let mut mgr = PluginManager::new(vec![]);
    mgr.register_provider(make_reg("lang", &["rs"], &[Capability::Fold]));
    let result = mgr.provider_for("rs", &Capability::Fold);
    assert!(
        result.is_some(),
        "provider must be found for registered ext"
    );
    assert_eq!(result.unwrap().plugin_name, "lang");
}

#[test]
fn provider_for_case_insensitive() {
    let mut mgr = PluginManager::new(vec![]);
    mgr.register_provider(make_reg("lang", &["rs"], &[Capability::Highlight]));
    assert!(
        mgr.provider_for("RS", &Capability::Highlight).is_some(),
        "lookup must be case-insensitive"
    );
}

#[test]
fn provider_for_wrong_capability_returns_none() {
    let mut mgr = PluginManager::new(vec![]);
    mgr.register_provider(make_reg("lang", &["rs"], &[Capability::Highlight]));
    assert!(
        mgr.provider_for("rs", &Capability::Fold).is_none(),
        "provider must not match when capability is absent"
    );
}

#[test]
fn provider_for_unregistered_ext_returns_none() {
    let mut mgr = PluginManager::new(vec![]);
    mgr.register_provider(make_reg("lang", &["rs"], &[Capability::Fold]));
    assert!(
        mgr.provider_for("py", &Capability::Fold).is_none(),
        "unregistered extension must return None"
    );
}

#[test]
fn register_provider_overwrites_same_plugin() {
    let mut mgr = PluginManager::new(vec![]);
    mgr.register_provider(make_reg("lang", &["rs"], &[Capability::Fold]));
    mgr.register_provider(make_reg("lang", &["py"], &[Capability::Fold]));
    assert!(
        mgr.provider_for("py", &Capability::Fold).is_some(),
        "re-registered extension must be present"
    );
    assert!(
        mgr.provider_for("rs", &Capability::Fold).is_none(),
        "old extension must be gone after re-registration"
    );
}
