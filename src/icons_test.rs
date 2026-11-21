use super::*;
use std::path::Path;

#[test]
fn table_is_sorted_and_unique_for_binary_search() {
    assert!(ICONS.windows(2).all(|w| w[0].0 < w[1].0));
}

#[test]
fn known_extensions_get_dedicated_glyphs() {
    assert_eq!(file_icon(Path::new("main.rs")), "\u{e7a8}");
    assert_eq!(file_icon(Path::new("a/b/App.PY")), "\u{e73c}");
    assert_eq!(file_icon(Path::new("README.md")), "\u{e73e}");
}

#[test]
fn extensionless_files_are_looked_up_by_name() {
    assert_eq!(file_icon(Path::new("Dockerfile")), "\u{e7b0}");
}

#[test]
fn unknown_types_use_the_fallback() {
    assert_eq!(file_icon(Path::new("data.unknownext")), FALLBACK);
    assert_eq!(file_icon(Path::new("Makefile")), FALLBACK);
}
