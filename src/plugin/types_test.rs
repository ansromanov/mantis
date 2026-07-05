use super::*;
use crate::plugin::types::{FromPlugin, ThemeColorsMsg, ToPlugin};
use crate::theme::{color_to_hex, Theme};

#[test]
fn plugin_entry_default_is_enabled_process() {
    let entry = PluginEntry::default();
    assert!(entry.enabled);
    assert_eq!(entry.kind, PluginKind::Process);
    assert!(entry.path.as_os_str().is_empty());
    assert!(entry.extensions.is_empty());
    assert!(entry.syntax_file.is_none());
    assert!(entry.events.is_empty());
}

#[test]
fn plugin_entry_events_deserialize_and_default_empty() {
    // `events` is optional in mantis.toml: absent => empty (all events).
    let without: PluginEntry =
        toml::from_str("path = \"p/run.sh\"\nenabled = true\n").expect("parse");
    assert!(without.events.is_empty());

    let with: PluginEntry = toml::from_str(
        "path = \"p/run.sh\"\nenabled = true\nevents = [\"on_file_open\", \"on_keypress\"]\n",
    )
    .expect("parse");
    assert_eq!(with.events, vec!["on_file_open", "on_keypress"]);
}

#[test]
fn plugin_kind_variants() {
    assert_eq!(PluginKind::Process as u8, 0);
    assert_eq!(PluginKind::Syntax as u8, 1);
}

#[test]
fn capability_variants_are_distinct() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(Capability::Highlight);
    set.insert(Capability::Fold);
    set.insert(Capability::Hover);
    set.insert(Capability::Diagnostics);
    set.insert(Capability::Definition);
    assert_eq!(set.len(), 5);
}

#[test]
fn language_provider_registration_holds_fields() {
    let caps: std::collections::HashSet<Capability> =
        [Capability::Highlight, Capability::Fold].into();
    let reg = LanguageProviderRegistration {
        plugin_name: "test-p".into(),
        extensions: vec!["rs".into(), "py".into()],
        capabilities: caps,
        priority: 0,
    };
    assert_eq!(reg.plugin_name, "test-p");
    assert_eq!(reg.extensions.len(), 2);
    assert!(reg.capabilities.contains(&Capability::Highlight));
    assert!(reg.capabilities.contains(&Capability::Fold));
    assert_eq!(reg.priority, 0);
}

#[test]
fn plugin_contributions_default_is_empty() {
    let c = PluginContributions::default();
    assert!(c.content_paths.is_empty());
    assert!(c.fold_region_paths.is_empty());
    assert!(!c.has_icon_map);
}

#[test]
fn plugin_contributions_tracks_inserted_paths() {
    let mut c = PluginContributions::default();
    let p = std::path::PathBuf::from("/tmp/file.rs");
    c.content_paths.insert(p.clone());
    c.has_icon_map = true;
    assert!(c.content_paths.contains(&p));
    assert!(c.has_icon_map);
}

#[test]
fn to_plugin_init_serializes_protocol_version() {
    let msg = ToPlugin {
        event: "init".into(),
        path: None,
        key: None,
        theme: Some("default".into()),
        colors: None,
        protocol_version: Some("2".into()),
        id: None,
        method: None,
        params: None,
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""event":"init""#));
    assert!(json.contains(r#""protocol_version":"2""#));
    // None fields are omitted.
    assert!(!json.contains(r#""path""#));
    assert!(!json.contains(r#""key""#));
}

#[test]
fn to_plugin_serializes_colors_when_present() {
    let msg = ToPlugin {
        event: "on_theme_change".into(),
        path: None,
        key: None,
        theme: Some("default".into()),
        colors: Some(ThemeColorsMsg::from(&Theme::default())),
        protocol_version: None,
        id: None,
        method: None,
        params: None,
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""colors":{"#));
    assert!(json.contains(r#""heading1":"#));
    assert!(json.contains(r#""text":"#));
}

#[test]
fn to_plugin_omits_colors_when_none() {
    let msg = ToPlugin {
        event: "on_keypress".into(),
        path: None,
        key: Some("q".into()),
        theme: None,
        colors: None,
        protocol_version: None,
        id: None,
        method: None,
        params: None,
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(!json.contains("colors"));
}

#[test]
fn theme_colors_msg_converts_real_theme_roles_to_hex() {
    let theme = Theme::default();
    let msg = ThemeColorsMsg::from(&theme);
    assert_eq!(msg.heading1, color_to_hex(theme.heading1));
    assert_eq!(msg.text, color_to_hex(theme.text));
}

#[test]
fn to_plugin_omits_protocol_version_when_none() {
    let msg = ToPlugin {
        event: "on_file_open".into(),
        path: Some("/a/b.rs".into()),
        key: None,
        theme: None,
        colors: None,
        protocol_version: None,
        id: None,
        method: None,
        params: None,
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(
        !json.contains("protocol_version"),
        "non-init events must not carry protocol_version"
    );
}

#[test]
fn extra_syntax_holds_path_and_extensions() {
    let sx = ExtraSyntax {
        syntax_path: "/a/b/c.sublime-syntax".into(),
        extensions: vec!["tf".into()],
    };
    assert_eq!(sx.syntax_path.to_string_lossy(), "/a/b/c.sublime-syntax");
    assert_eq!(sx.extensions, vec!["tf"]);
}

// -- protocol 3: request/response wire format ---------------------------------

#[test]
fn to_plugin_request_serializes_id_method_params() {
    let msg = ToPlugin {
        event: "request".into(),
        path: None,
        key: None,
        theme: None,
        colors: None,
        protocol_version: None,
        id: Some(42),
        method: Some("fold_regions".into()),
        params: Some(serde_json::json!({"path": "/a/b.rs"})),
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""event":"request""#));
    assert!(json.contains(r#""id":42"#));
    assert!(json.contains(r#""method":"fold_regions""#));
    assert!(json.contains(r#""path":"/a/b.rs""#));
}

#[test]
fn to_plugin_omits_request_fields_when_none() {
    let msg = ToPlugin {
        event: "on_file_open".into(),
        path: Some("/a/b.rs".into()),
        key: None,
        theme: None,
        colors: None,
        protocol_version: None,
        id: None,
        method: None,
        params: None,
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(!json.contains(r#""id""#));
    assert!(!json.contains(r#""method""#));
}

#[test]
fn from_plugin_deserializes_response_with_result() {
    let json = r#"{"event":"response","id":7,"result":{"regions":[[0,5]]}}"#;
    let msg: FromPlugin = serde_json::from_str(json).unwrap();
    assert_eq!(msg.event, "response");
    assert_eq!(msg.id, Some(7));
    assert!(msg.result.is_some());
    assert!(msg.error.is_none());
}

#[test]
fn from_plugin_deserializes_response_with_error() {
    let json = r#"{"event":"response","id":7,"error":{"message":"boom"}}"#;
    let msg: FromPlugin = serde_json::from_str(json).unwrap();
    assert_eq!(msg.id, Some(7));
    assert_eq!(msg.error.unwrap().message, "boom");
    assert!(msg.result.is_none());
}

#[test]
fn from_plugin_action_message_leaves_response_fields_none() {
    let json = r#"{"event":"action","action":"show_message","params":{"message":"hi"}}"#;
    let msg: FromPlugin = serde_json::from_str(json).unwrap();
    assert_eq!(msg.action.as_deref(), Some("show_message"));
    assert!(msg.id.is_none());
    assert!(msg.result.is_none());
    assert!(msg.error.is_none());
}
// Satisfying require-tests check
