use crate::plugin::process::Plugin;
use crate::plugin::types::ToPlugin;

#[test]
fn plugin_new_sets_name() {
    let p = Plugin::new("test-p".into());
    assert_eq!(p.name, "test-p");
}

#[test]
fn drain_actions_returns_empty_when_no_reader() {
    let mut p = Plugin::new("no-reader".into());
    assert!(p.drain_actions().is_empty());
}

#[test]
fn send_is_noop_when_no_writer() {
    let mut p = Plugin::new("no-writer".into());
    p.send(&ToPlugin {
        event: "test".into(),
        path: None,
        key: None,
        theme: None,
        protocol_version: None,
    });
}
