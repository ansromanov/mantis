use std::path::PathBuf;

use super::*;
use crate::theme::{color_to_hex, Theme};

fn make_reg(name: &str, exts: &[&str], caps: &[Capability]) -> LanguageProviderRegistration {
    make_reg_priority(name, exts, caps, 0)
}

fn make_reg_priority(
    name: &str,
    exts: &[&str],
    caps: &[Capability],
    priority: i64,
) -> LanguageProviderRegistration {
    LanguageProviderRegistration {
        plugin_name: name.to_string(),
        extensions: exts.iter().map(|e| e.to_string()).collect(),
        capabilities: caps.iter().cloned().collect(),
        priority,
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
        mgr.plugin_entries()
            .iter()
            .filter(|(_, r, _, _)| *r)
            .count(),
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
        init_line.contains(r#""protocol_version":"3""#),
        "init must carry host protocol version, got: {init_line}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// Spawns a recording-stub plugin via `activate_all`, sends it `event`, and
/// returns the first matching line it received on stdin. Shared by the
/// `activate_all`/`on_theme_change` colors tests below.
#[cfg(unix)]
fn record_event_from_activate_all(theme: &Theme, event: &str) -> String {
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    let dir = std::env::temp_dir().join(format!("tv_mgr_colors_{event}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("recv.txt");

    let script = dir.join("rec.sh");
    let mut f = std::fs::File::create(&script).unwrap();
    write!(f, "#!/bin/sh\ncat > \"{}\"\n", out.display()).unwrap();
    drop(f);
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let entry = PluginEntry {
        path: script.clone(),
        enabled: true,
        ..Default::default()
    };
    let mut mgr = PluginManager::new(vec![("rec".to_string(), entry)]);
    mgr.activate_all(Some("custom"), theme);
    mgr.deactivate_all();

    let deadline = Instant::now() + Duration::from_secs(3);
    let contents = loop {
        if let Ok(s) = std::fs::read_to_string(&out) {
            if !s.is_empty() {
                break s;
            }
        }
        assert!(Instant::now() < deadline, "plugin never received {event}");
        std::thread::sleep(Duration::from_millis(25));
    };
    let line = contents
        .lines()
        .find(|l| l.contains(&format!(r#""event":"{event}""#)))
        .unwrap_or_else(|| panic!("{event} event must be sent, got: {contents}"))
        .to_string();
    std::fs::remove_dir_all(&dir).ok();
    line
}

#[test]
#[cfg(unix)]
fn activate_all_sends_init_with_theme_colors() {
    let theme = Theme::default();
    let init_line = record_event_from_activate_all(&theme, "init");
    let expected_heading1 = format!(r#""heading1":"{}""#, color_to_hex(theme.heading1));
    assert!(
        init_line.contains(&expected_heading1),
        "init colors must carry the theme's actual heading1 hex, got: {init_line}"
    );
}

#[test]
#[cfg(unix)]
fn on_theme_change_sends_updated_theme_colors() {
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    let dir = std::env::temp_dir().join(format!("tv_mgr_theme_change_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("recv.txt");

    let script = dir.join("rec.sh");
    let mut f = std::fs::File::create(&script).unwrap();
    write!(f, "#!/bin/sh\ncat > \"{}\"\n", out.display()).unwrap();
    drop(f);
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let entry = PluginEntry {
        path: script.clone(),
        enabled: true,
        ..Default::default()
    };
    let mut mgr = PluginManager::new(vec![("rec".to_string(), entry)]);
    mgr.activate_all(Some("default"), &Theme::default());

    let monokai = Theme::load("monokai").expect("monokai theme must load");
    mgr.on_theme_change("monokai", &monokai);
    mgr.deactivate_all();

    let deadline = Instant::now() + Duration::from_secs(3);
    let contents = loop {
        if let Ok(s) = std::fs::read_to_string(&out) {
            if s.matches(r#""event":"on_theme_change""#).count() >= 1 {
                break s;
            }
        }
        assert!(
            Instant::now() < deadline,
            "plugin never received on_theme_change"
        );
        std::thread::sleep(Duration::from_millis(25));
    };
    let line = contents
        .lines()
        .find(|l| l.contains(r#""event":"on_theme_change""#))
        .expect("on_theme_change event must be sent");
    let expected_heading1 = format!(r#""heading1":"{}""#, color_to_hex(monokai.heading1));
    assert!(
        line.contains(&expected_heading1),
        "on_theme_change colors must reflect the new theme, got: {line}"
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
fn event_dispatch_with_no_active_plugins_is_noop() {
    // The per-event `subscribes_to` gate runs inside a loop over active
    // plugins; with none spawned every dispatch must be a harmless noop.
    let mut mgr = PluginManager::new(vec![]);
    mgr.on_file_open(std::path::Path::new("/tmp/x.rs"));
    mgr.on_theme_change("dark", &crate::theme::Theme::default());
    mgr.on_selection_change(Some(std::path::Path::new("/tmp/x.rs")));
    mgr.on_quit();
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

/// Shared crate-wide lock serialising every test that sets `MANTIS_STATE_DIR`
/// (a process-global env var) — see [`crate::session::STATE_DIR_ENV_LOCK`].
#[cfg(unix)]
use crate::session::STATE_DIR_ENV_LOCK as STATE_DIR_LOCK;

#[test]
#[cfg(unix)]
fn drain_actions_records_crash_diagnostics_and_picker_badge() {
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    let _lock = STATE_DIR_LOCK.lock().unwrap();

    let dir = std::env::temp_dir().join(format!("tv_mgr_crash_{}", std::process::id()));
    let state_dir = dir.join("state");
    std::fs::create_dir_all(&state_dir).unwrap();
    std::env::set_var("MANTIS_STATE_DIR", &state_dir);

    let script = dir.join("crash.sh");
    let mut f = std::fs::File::create(&script).unwrap();
    write!(f, "#!/bin/sh\necho 'panic: oh no' >&2\nexit 1\n").unwrap();
    drop(f);
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let entry = PluginEntry {
        path: script.clone(),
        enabled: false,
        ..Default::default()
    };
    let mut mgr = PluginManager::new(vec![("crashy".to_string(), entry)]);
    mgr.activate_one("crashy", None).expect("spawn crash.sh");

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        mgr.drain_actions();
        if mgr.crash_detail("crashy").is_some() {
            break;
        }
        assert!(Instant::now() < deadline, "plugin never detected as dead");
        std::thread::sleep(Duration::from_millis(25));
    }

    let detail = mgr.crash_detail("crashy").expect("crash detail recorded");
    assert_eq!(detail.last_stderr.as_deref(), Some("panic: oh no"));
    assert!(detail.log_path.is_some(), "log path must be recorded");

    let badge = mgr
        .plugin_entries()
        .into_iter()
        .find(|(name, _, _, _)| name == "crashy")
        .and_then(|(_, _, _, badge)| badge)
        .expect("dead plugin must show a crash badge in the picker");
    assert!(
        badge.contains("panic: oh no"),
        "badge must surface the last stderr line, got: {badge}"
    );

    std::env::remove_var("MANTIS_STATE_DIR");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn take_dead_plugins_empty_by_default() {
    let mut mgr = PluginManager::new(vec![]);
    assert!(
        mgr.take_dead_plugins().is_empty(),
        "no plugins have died on a fresh manager"
    );
}

#[test]
fn remove_provider_registrations_removes_only_named_plugin() {
    let mut mgr = PluginManager::new(vec![]);
    mgr.register_provider(make_reg("keep", &["rs"], &[Capability::Fold]));
    mgr.register_provider(make_reg("drop", &["py"], &[Capability::Fold]));

    mgr.remove_provider_registrations("drop");

    assert!(
        mgr.provider_for("rs", &Capability::Fold).is_some(),
        "untouched plugin's registration must remain"
    );
    assert!(
        mgr.provider_for("py", &Capability::Fold).is_none(),
        "removed plugin's registration must be gone"
    );
}

#[test]
fn remove_provider_registrations_unknown_name_is_noop() {
    let mut mgr = PluginManager::new(vec![]);
    mgr.register_provider(make_reg("keep", &["rs"], &[Capability::Fold]));
    mgr.remove_provider_registrations("never-registered");
    assert!(
        mgr.provider_for("rs", &Capability::Fold).is_some(),
        "removing an unknown plugin must not drop other registrations"
    );
}

// -- protocol 3: provider priority ---------------------------------------------

#[test]
fn provider_for_prefers_higher_priority_regardless_of_registration_order() {
    let mut mgr = PluginManager::new(vec![]);
    mgr.register_provider(make_reg_priority("low", &["rs"], &[Capability::Fold], 1));
    mgr.register_provider(make_reg_priority("high", &["rs"], &[Capability::Fold], 10));
    let result = mgr.provider_for("rs", &Capability::Fold).unwrap();
    assert_eq!(result.plugin_name, "high");
}

#[test]
fn provider_for_high_priority_registered_first_still_wins() {
    let mut mgr = PluginManager::new(vec![]);
    mgr.register_provider(make_reg_priority("high", &["rs"], &[Capability::Fold], 10));
    mgr.register_provider(make_reg_priority("low", &["rs"], &[Capability::Fold], 1));
    let result = mgr.provider_for("rs", &Capability::Fold).unwrap();
    assert_eq!(result.plugin_name, "high");
}

#[test]
fn provider_for_equal_priority_keeps_first_registered() {
    let mut mgr = PluginManager::new(vec![]);
    mgr.register_provider(make_reg_priority("first", &["rs"], &[Capability::Fold], 5));
    mgr.register_provider(make_reg_priority("second", &["rs"], &[Capability::Fold], 5));
    let result = mgr.provider_for("rs", &Capability::Fold).unwrap();
    assert_eq!(result.plugin_name, "first");
}

#[test]
fn register_provider_warns_once_on_conflict() {
    let mut mgr = PluginManager::new(vec![]);
    let warning1 = mgr.register_provider(make_reg("first", &["rs"], &[Capability::Fold]));
    assert!(warning1.is_none(), "first registration has no conflict");

    let warning2 = mgr.register_provider(make_reg("second", &["rs"], &[Capability::Fold]));
    let warning2 = warning2.expect("conflicting registration must warn");
    assert!(warning2.contains("first"));
    assert!(warning2.contains("second"));

    // A third conflicting registration for the same pair must not warn again.
    let warning3 = mgr.register_provider(make_reg("third", &["rs"], &[Capability::Fold]));
    assert!(
        warning3.is_none(),
        "conflict warning must only fire once per (ext, capability) pair"
    );
}

#[test]
fn register_provider_no_conflict_for_different_capability() {
    let mut mgr = PluginManager::new(vec![]);
    mgr.register_provider(make_reg("first", &["rs"], &[Capability::Fold]));
    let warning = mgr.register_provider(make_reg("second", &["rs"], &[Capability::Highlight]));
    assert!(
        warning.is_none(),
        "different capability for the same extension must not conflict"
    );
}

#[test]
fn register_provider_re_registration_of_same_plugin_is_not_a_conflict() {
    let mut mgr = PluginManager::new(vec![]);
    mgr.register_provider(make_reg("solo", &["rs"], &[Capability::Fold]));
    let warning = mgr.register_provider(make_reg_priority("solo", &["rs"], &[Capability::Fold], 5));
    assert!(
        warning.is_none(),
        "re-registering the same plugin must not self-conflict"
    );
}

// -- protocol 3: plugin_error diagnostics --------------------------------------

#[test]
fn record_plugin_error_and_plugin_error_for_roundtrip() {
    let mut mgr = PluginManager::new(vec![]);
    assert!(mgr.plugin_error_for("x").is_none());
    mgr.record_plugin_error("x", "oops".into(), None);
    let info = mgr.plugin_error_for("x").expect("recorded");
    assert_eq!(info.message, "oops");
    assert!(info.context.is_none());
}

#[test]
#[cfg(unix)]
fn plugin_entries_badges_active_plugin_with_recorded_error() {
    let entry = PluginEntry {
        path: PathBuf::from("/bin/cat"),
        enabled: false,
        ..Default::default()
    };
    let mut mgr = PluginManager::new(vec![("noisy".to_string(), entry)]);
    mgr.activate_one("noisy", None).expect("spawn /bin/cat");
    mgr.record_plugin_error(
        "noisy",
        "bad thing happened".into(),
        Some("on_file_open".into()),
    );

    let badge = mgr
        .plugin_entries()
        .into_iter()
        .find(|(name, _, _, _)| name == "noisy")
        .map(|(_, active, _, badge)| {
            assert!(active, "plugin_error must not mark the plugin dead");
            badge
        })
        .and_then(|badge| badge)
        .expect("running plugin with a recorded error must show a badge");
    assert!(badge.contains("bad thing happened"));
    assert!(badge.contains("on_file_open"));

    mgr.deactivate_all();
}

// -- protocol 3: on_keypress subscriber gate ------------------------------------

#[test]
fn has_keypress_subscriber_false_when_no_plugins() {
    let mgr = PluginManager::new(vec![]);
    assert!(!mgr.has_keypress_subscriber());
}

#[test]
#[cfg(unix)]
fn has_keypress_subscriber_true_when_a_plugin_subscribes() {
    let entry = PluginEntry {
        path: PathBuf::from("/bin/cat"),
        enabled: false,
        events: vec!["on_keypress".to_string()],
        ..Default::default()
    };
    let mut mgr = PluginManager::new(vec![("kp".to_string(), entry)]);
    mgr.activate_one("kp", None).expect("spawn /bin/cat");
    assert!(mgr.has_keypress_subscriber());
    mgr.deactivate_all();
}

#[test]
#[cfg(unix)]
fn has_keypress_subscriber_false_when_plugin_subscribes_to_other_events_only() {
    let entry = PluginEntry {
        path: PathBuf::from("/bin/cat"),
        enabled: false,
        events: vec!["on_file_open".to_string()],
        ..Default::default()
    };
    let mut mgr = PluginManager::new(vec![("fo".to_string(), entry)]);
    mgr.activate_one("fo", None).expect("spawn /bin/cat");
    assert!(!mgr.has_keypress_subscriber());
    mgr.deactivate_all();
}

#[test]
#[cfg(unix)]
fn has_keypress_subscriber_false_for_wildcard_plugin_with_no_events_filter() {
    // An empty `events` list is the back-compat wildcard ("deliver every
    // event to me"), not an opt-in to key consumption. A plugin like this
    // (e.g. the bundled iconize/markdown plugins) never replies
    // `key_handled` and must not delay every keystroke in the host waiting
    // for one.
    let entry = PluginEntry {
        path: PathBuf::from("/bin/cat"),
        enabled: false,
        events: vec![],
        ..Default::default()
    };
    let mut mgr = PluginManager::new(vec![("wild".to_string(), entry)]);
    mgr.activate_one("wild", None).expect("spawn /bin/cat");
    assert!(!mgr.has_keypress_subscriber());
    mgr.deactivate_all();
}

// -- protocol 3: request/response ------------------------------------------

#[test]
fn send_request_returns_none_for_unknown_plugin() {
    let mut mgr = PluginManager::new(vec![]);
    assert!(mgr
        .send_request("ghost", "fold_regions", serde_json::json!({}))
        .is_none());
}

#[test]
fn poll_requests_empty_when_nothing_pending() {
    let mut mgr = PluginManager::new(vec![]);
    assert!(mgr.poll_requests().is_empty());
}

#[test]
#[cfg(unix)]
fn send_request_and_poll_requests_matches_response_by_id() {
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    let dir = std::env::temp_dir().join(format!("tv_mgr_request_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    // Stub plugin: for every stdin line that looks like a `request` event,
    // extract its `id` (via shell parameter expansion, no subprocess spawn,
    // so the response is fast enough to beat REQUEST_TIMEOUT) and echo back
    // a matching `response`.
    let script = dir.join("respond.sh");
    let mut f = std::fs::File::create(&script).unwrap();
    write!(
        f,
        "#!/bin/sh\nwhile read -r line; do\n  case \"$line\" in\n    *'\"event\":\"request\"'*)\n      rest=${{line#*\\\"id\\\":}}\n      id=${{rest%%,*}}\n      echo \"{{\\\"event\\\":\\\"response\\\",\\\"id\\\":$id,\\\"result\\\":{{\\\"ok\\\":true}}}}\"\n      ;;\n  esac\ndone\n"
    )
    .unwrap();
    drop(f);
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let entry = PluginEntry {
        path: script.clone(),
        enabled: false,
        ..Default::default()
    };
    let mut mgr = PluginManager::new(vec![("responder".to_string(), entry)]);
    mgr.activate_one("responder", None)
        .expect("spawn respond.sh");

    let id = mgr
        .send_request(
            "responder",
            "fold_regions",
            serde_json::json!({"path": "/a.rs"}),
        )
        .expect("plugin is running, request must be sent");

    // Deadline comfortably exceeds REQUEST_TIMEOUT (2s under cfg(test)) so a
    // slow response under parallel test-suite load is still matched rather
    // than raced against the timeout path.
    let deadline = Instant::now() + Duration::from_secs(5);
    let results = loop {
        let results = mgr.poll_requests();
        if !results.is_empty() {
            break results;
        }
        assert!(Instant::now() < deadline, "response never matched");
        std::thread::sleep(Duration::from_millis(25));
    };
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, id);
    assert!(results[0].1.is_ok());

    mgr.deactivate_all();
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
#[cfg(unix)]
fn send_request_times_out_without_response() {
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    let dir = std::env::temp_dir().join(format!("tv_mgr_request_timeout_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    // Stub plugin that reads and discards everything, never responding.
    let script = dir.join("silent.sh");
    let mut f = std::fs::File::create(&script).unwrap();
    write!(f, "#!/bin/sh\ncat > /dev/null\n").unwrap();
    drop(f);
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let entry = PluginEntry {
        path: script.clone(),
        enabled: false,
        ..Default::default()
    };
    let mut mgr = PluginManager::new(vec![("silent".to_string(), entry)]);
    mgr.activate_one("silent", None).expect("spawn silent.sh");

    let id = mgr
        .send_request("silent", "fold_regions", serde_json::json!({}))
        .expect("request must be sent");

    // REQUEST_TIMEOUT is 2s under cfg(test); wait comfortably past it.
    let deadline = Instant::now() + Duration::from_secs(5);
    let results = loop {
        let results = mgr.poll_requests();
        if !results.is_empty() {
            break results;
        }
        assert!(Instant::now() < deadline, "timeout never fired");
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(results[0].0, id);
    assert!(results[0].1.is_err());
    assert!(
        mgr.plugin_error_for("silent").is_some(),
        "a timed-out request must be recorded like a plugin_error"
    );

    mgr.deactivate_all();
    std::fs::remove_dir_all(&dir).ok();
}
