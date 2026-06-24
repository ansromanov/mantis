use super::*;
use crate::plugin::types::ToPlugin;

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
    // `events` is optional in tv.toml: absent => empty (all events).
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
    };
    assert_eq!(reg.plugin_name, "test-p");
    assert_eq!(reg.extensions.len(), 2);
    assert!(reg.capabilities.contains(&Capability::Highlight));
    assert!(reg.capabilities.contains(&Capability::Fold));
}

#[test]
fn plugin_contributions_default_is_empty() {
    let c = PluginContributions::default();
    assert!(c.content_paths.is_empty());
    assert!(c.blame_paths.is_empty());
    assert!(c.status_paths.is_empty());
    assert!(c.fold_region_paths.is_empty());
    assert!(!c.has_git_info);
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
    // Independent sets stay empty.
    assert!(c.blame_paths.is_empty());
}

#[test]
fn to_plugin_init_serializes_protocol_version() {
    let msg = ToPlugin {
        event: "init".into(),
        path: None,
        key: None,
        theme: Some("default".into()),
        protocol_version: Some("2".into()),
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""event":"init""#));
    assert!(json.contains(r#""protocol_version":"2""#));
    // None fields are omitted.
    assert!(!json.contains(r#""path""#));
    assert!(!json.contains(r#""key""#));
}

#[test]
fn to_plugin_omits_protocol_version_when_none() {
    let msg = ToPlugin {
        event: "on_file_open".into(),
        path: Some("/a/b.rs".into()),
        key: None,
        theme: None,
        protocol_version: None,
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
