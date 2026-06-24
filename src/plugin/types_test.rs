use super::*;

#[test]
fn plugin_entry_default_is_enabled_process() {
    let entry = PluginEntry::default();
    assert!(entry.enabled);
    assert_eq!(entry.kind, PluginKind::Process);
    assert!(entry.path.as_os_str().is_empty());
    assert!(entry.extensions.is_empty());
    assert!(entry.syntax_file.is_none());
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
fn extra_syntax_holds_path_and_extensions() {
    let sx = ExtraSyntax {
        syntax_path: "/a/b/c.sublime-syntax".into(),
        extensions: vec!["tf".into()],
    };
    assert_eq!(sx.syntax_path.to_string_lossy(), "/a/b/c.sublime-syntax");
    assert_eq!(sx.extensions, vec!["tf"]);
}
