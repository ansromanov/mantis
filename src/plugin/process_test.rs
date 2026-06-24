use crate::plugin::process::Plugin;
use crate::plugin::types::ToPlugin;

#[test]
fn plugin_new_sets_name() {
    let p = Plugin::new("test-p".into(), vec![]);
    assert_eq!(p.name, "test-p");
}

#[test]
fn plugin_new_stores_subscribed_events() {
    let p = Plugin::new("evt-p".into(), vec!["on_file_open".into()]);
    assert!(p.subscribes_to("on_file_open"));
    assert!(!p.subscribes_to("on_keypress"));
}

#[test]
fn plugin_new_empty_events_subscribes_to_all() {
    let p = Plugin::new("all-p".into(), vec![]);
    assert!(p.subscribes_to("on_file_open"));
    assert!(p.subscribes_to("on_keypress"));
    assert!(p.subscribes_to("on_quit"));
}

#[test]
fn drain_actions_returns_empty_when_no_reader() {
    let mut p = Plugin::new("no-reader".into(), vec![]);
    assert!(p.drain_actions().is_empty());
}

#[test]
fn send_is_noop_when_no_writer() {
    let mut p = Plugin::new("no-writer".into(), vec![]);
    p.send(&ToPlugin {
        event: "test".into(),
        path: None,
        key: None,
        theme: None,
    });
}
