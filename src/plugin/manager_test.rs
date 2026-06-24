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
