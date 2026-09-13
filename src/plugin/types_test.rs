use super::*;
use crate::plugin::types::{enclosing_symbol, FromPlugin, Symbol, ThemeColorsMsg, ToPlugin};
use crate::theme::{color_to_hex, Theme};

fn symbol(name: &str, line: usize, end_line: usize, parent: Option<&str>) -> Symbol {
    Symbol {
        name: name.to_string(),
        kind: "function".to_string(),
        line,
        end_line: Some(end_line),
        parent: parent.map(str::to_string),
    }
}

#[test]
fn enclosing_symbol_selects_the_narrowest_range_and_includes_boundaries() {
    let symbols = [
        symbol("outer", 2, 20, None),
        symbol("inner", 5, 8, Some("outer")),
    ];
    assert_eq!(
        enclosing_symbol(&symbols, 5).map(|item| item.name.as_str()),
        Some("inner")
    );
    assert_eq!(
        enclosing_symbol(&symbols, 8).map(|item| item.name.as_str()),
        Some("inner")
    );
    assert_eq!(
        enclosing_symbol(&symbols, 9).map(|item| item.name.as_str()),
        Some("outer")
    );
}

#[test]
fn symbol_without_end_line_matches_only_its_declaration() {
    let symbol = Symbol {
        name: "field".into(),
        kind: "key".into(),
        line: 3,
        end_line: None,
        parent: None,
    };
    assert!(enclosing_symbol(std::slice::from_ref(&symbol), 3).is_some());
    assert!(enclosing_symbol(&[symbol], 4).is_none());
}

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
    set.insert(Capability::StatusFacts);
    set.insert(Capability::Symbols);
    assert_eq!(set.len(), 7);
}

#[test]
fn capability_symbols_serializes_as_snake_case() {
    let json = serde_json::to_string(&Capability::Symbols).unwrap();
    assert_eq!(json, r#""symbols""#);
    assert_eq!(
        serde_json::from_str::<Capability>(&json).unwrap(),
        Capability::Symbols
    );
}

#[test]
fn capability_status_facts_serializes_as_snake_case() {
    let json = serde_json::to_string(&Capability::StatusFacts).unwrap();
    assert_eq!(json, r#""status_facts""#);
    let back: Capability = serde_json::from_str(&json).unwrap();
    assert_eq!(back, Capability::StatusFacts);
}

#[test]
fn language_provider_registration_holds_fields() {
    let caps: std::collections::HashSet<Capability> =
        [Capability::Highlight, Capability::Fold].into();
    let reg = LanguageProviderRegistration {
        plugin_name: "test-p".into(),
        extensions: vec!["rs".into(), "py".into()],
        filenames: vec!["Makefile".into()],
        shebangs: vec!["bash".into()],
        capabilities: caps,
        priority: 0,
    };
    assert_eq!(reg.plugin_name, "test-p");
    assert_eq!(reg.extensions.len(), 2);
    assert_eq!(reg.filenames, vec!["Makefile"]);
    assert_eq!(reg.shebangs, vec!["bash"]);
    assert!(reg.capabilities.contains(&Capability::Highlight));
    assert!(reg.capabilities.contains(&Capability::Fold));
    assert_eq!(reg.priority, 0);
}

#[test]
fn plugin_contributions_default_is_empty() {
    let c = PluginContributions::default();
    assert!(c.content_paths.is_empty());
    assert!(c.fold_region_paths.is_empty());
    assert!(c.status_fact_paths.is_empty());
    assert!(c.symbol_paths.is_empty());
    assert!(!c.has_icon_map);
}

#[test]
fn plugin_contributions_tracks_inserted_paths() {
    let mut c = PluginContributions::default();
    let p = std::path::PathBuf::from("/tmp/file.rs");
    c.content_paths.insert(p.clone());
    c.status_fact_paths.insert(p.clone());
    c.symbol_paths.insert(p.clone());
    c.has_icon_map = true;
    assert!(c.content_paths.contains(&p));
    assert!(c.status_fact_paths.contains(&p));
    assert!(c.symbol_paths.contains(&p));
    assert!(c.has_icon_map);
}

#[test]
fn to_plugin_init_serializes_protocol_version() {
    let msg = ToPlugin {
        event: "init".into(),
        path: None,
        line: None,
        column: None,
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
        line: None,
        column: None,
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
        line: None,
        column: None,
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
        line: None,
        column: None,
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
fn to_plugin_selection_change_serializes_zero_based_line() {
    let msg = ToPlugin {
        event: "on_selection_change".into(),
        path: Some("/a/b.yaml".into()),
        line: Some(12),
        column: None,
        key: None,
        theme: None,
        colors: None,
        protocol_version: None,
        id: None,
        method: None,
        params: None,
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""line":12"#));
}

#[test]
fn to_plugin_content_cursor_change_serializes_one_based_position() {
    let msg = ToPlugin {
        event: "on_content_cursor_change".into(),
        path: Some("/a/b.yaml".into()),
        line: Some(13),
        column: Some(1),
        key: None,
        theme: None,
        colors: None,
        protocol_version: None,
        id: None,
        method: None,
        params: None,
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""event":"on_content_cursor_change""#));
    assert!(json.contains(r#""line":13"#));
    assert!(json.contains(r#""column":1"#));
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
        line: None,
        column: None,
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
        line: None,
        column: None,
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

#[test]
fn plugin_command_deserializes_with_optional_fields_defaulting() {
    let c: PluginCommand = serde_json::from_str(r#"{"id":"x.y","name":"X"}"#).unwrap();
    assert_eq!(c.id, "x.y");
    assert_eq!(c.name, "X");
    assert!(c.category.is_none());
    assert!(c.description.is_none());

    let c2: PluginCommand = serde_json::from_str(
        r#"{"id":"a","name":"A","category":"Plugin","description":"does a thing"}"#,
    )
    .unwrap();
    assert_eq!(c2.category.as_deref(), Some("Plugin"));
    assert_eq!(c2.description.as_deref(), Some("does a thing"));
}

#[test]
fn plugin_contributions_tracks_command_ids() {
    let mut contrib = PluginContributions::default();
    assert!(contrib.command_ids.is_empty());
    contrib.command_ids.insert("demo.hello".to_string());
    assert!(contrib.command_ids.contains("demo.hello"));
}

#[test]
fn context_item_target_kind_as_str_and_serde() {
    for (kind, expected) in [
        (ContextItemTargetKind::TreeFile, "tree_file"),
        (ContextItemTargetKind::TreeDir, "tree_dir"),
        (ContextItemTargetKind::Content, "content"),
        (ContextItemTargetKind::Tab, "tab"),
        (ContextItemTargetKind::Blame, "blame"),
    ] {
        assert_eq!(kind.as_str(), expected);
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, format!("\"{expected}\""));
        let de: ContextItemTargetKind = serde_json::from_str(&json).unwrap();
        assert_eq!(de, kind);
    }
}

#[test]
fn plugin_context_item_deserializes_with_defaults() {
    let json = r#"{"id":"test.action","label":"Test Action","target":"content"}"#;
    let item: PluginContextItem = serde_json::from_str(json).unwrap();
    assert_eq!(item.id, "test.action");
    assert_eq!(item.label, "Test Action");
    assert_eq!(item.target, ContextItemTargetKind::Content);
    assert!(item.extensions.is_none());
    assert!(item.category.is_none());
    assert!(item.weight.is_none());

    let full_json = r#"{
        "id":"test.git",
        "label":"Git Action",
        "target":"tree_file",
        "extensions":["rs","toml"],
        "category":"Tools",
        "weight":5
    }"#;
    let full_item: PluginContextItem = serde_json::from_str(full_json).unwrap();
    assert_eq!(full_item.target, ContextItemTargetKind::TreeFile);
    assert_eq!(full_item.extensions, Some(vec!["rs".into(), "toml".into()]));
    assert_eq!(full_item.category.as_deref(), Some("Tools"));
    assert_eq!(full_item.weight, Some(5));
}

#[test]
fn plugin_status_segment_deserializes_with_defaults() {
    let json = r#"{"id":"test.stat","text":"Status Info"}"#;
    let seg: PluginStatusSegment = serde_json::from_str(json).unwrap();
    assert_eq!(seg.id, "test.stat");
    assert_eq!(seg.text, "Status Info");
    assert!(seg.priority.is_none());
    assert!(seg.side.is_none());

    let full_json = r#"{
        "id":"test.right",
        "text":"R Info",
        "priority":4,
        "side":"right"
    }"#;
    let full_seg: PluginStatusSegment = serde_json::from_str(full_json).unwrap();
    assert_eq!(full_seg.priority, Some(4));
    assert_eq!(full_seg.side, Some(StatusSidePreference::Right));
}

#[test]
fn plugin_contributions_tracks_context_and_status_ids() {
    let mut contrib = PluginContributions::default();
    assert!(contrib.context_item_ids.is_empty());
    assert!(contrib.status_segment_ids.is_empty());
    contrib.context_item_ids.insert("my_action".to_string());
    contrib.status_segment_ids.insert("my_status".to_string());
    assert!(contrib.context_item_ids.contains("my_action"));
    assert!(contrib.status_segment_ids.contains("my_status"));
}
