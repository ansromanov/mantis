//! Nerd Font file-type icons for the tree (`tree.icons = true`).
//!
//! A static extension to glyph table. This replaces the bundled `iconize`
//! plugin, which sent the same table over IPC at startup. Lookup is by
//! lowercase extension, or by lowercase file name for extension-less files
//! (e.g. `dockerfile`). Requires a Nerd Font in the terminal.
//!
//! Public items:
//! - [`DIR_OPEN`], [`DIR_CLOSED`], [`FALLBACK`]: glyphs for folders and unknown types.
//! - [`file_icon`]: glyph for a file name.

use std::path::Path;

/// Glyph for an expanded directory.
pub const DIR_OPEN: &str = "\u{f07c}";
/// Glyph for a collapsed directory.
pub const DIR_CLOSED: &str = "\u{f07b}";
/// Glyph for file types without a dedicated icon.
pub const FALLBACK: &str = "\u{f15b}";

/// Extension (lowercase) to glyph, sorted for binary search.
const ICONS: &[(&str, &str)] = &[
    ("7z", "\u{f1c6}"),
    ("avi", "\u{f03d}"),
    ("bash", "\u{e795}"),
    ("bin", "\u{e70f}"),
    ("bz2", "\u{f1c6}"),
    ("c", "\u{e79b}"),
    ("clj", "\u{e768}"),
    ("cljs", "\u{e768}"),
    ("cpp", "\u{e61d}"),
    ("cs", "\u{e77f}"),
    ("css", "\u{e749}"),
    ("dart", "\u{e798}"),
    ("db", "\u{e706}"),
    ("dll", "\u{e70f}"),
    ("dockerfile", "\u{e7b0}"),
    ("dylib", "\u{e70f}"),
    ("eot", "\u{f031}"),
    ("epub", "\u{f1c1}"),
    ("erl", "\u{e7b1}"),
    ("ex", "\u{e62d}"),
    ("exe", "\u{e70f}"),
    ("fish", "\u{e795}"),
    ("flac", "\u{f001}"),
    ("gif", "\u{f1c5}"),
    ("go", "\u{e627}"),
    ("gql", "\u{e844}"),
    ("graphql", "\u{e844}"),
    ("gz", "\u{f1c6}"),
    ("hs", "\u{e61f}"),
    ("htm", "\u{e736}"),
    ("html", "\u{e736}"),
    ("ico", "\u{f1c5}"),
    ("java", "\u{e738}"),
    ("jpeg", "\u{f1c5}"),
    ("jpg", "\u{f1c5}"),
    ("js", "\u{e74e}"),
    ("json", "\u{e60b}"),
    ("kt", "\u{e634}"),
    ("kts", "\u{e634}"),
    ("less", "\u{e758}"),
    ("lock", "\u{e6c6}"),
    ("lua", "\u{e620}"),
    ("m4a", "\u{f001}"),
    ("markdown", "\u{e73e}"),
    ("md", "\u{e73e}"),
    ("mkv", "\u{f03d}"),
    ("mobi", "\u{f1c1}"),
    ("mov", "\u{f03d}"),
    ("mp3", "\u{f001}"),
    ("mp4", "\u{f03d}"),
    ("node", "\u{e718}"),
    ("ogg", "\u{f001}"),
    ("otf", "\u{f031}"),
    ("pdf", "\u{f1c1}"),
    ("php", "\u{e73d}"),
    ("png", "\u{f1c5}"),
    ("py", "\u{e73c}"),
    ("r", "\u{e71c}"),
    ("rar", "\u{f1c6}"),
    ("rb", "\u{e739}"),
    ("rmd", "\u{e71c}"),
    ("rs", "\u{e7a8}"),
    ("scala", "\u{e737}"),
    ("scss", "\u{e74b}"),
    ("sh", "\u{e795}"),
    ("so", "\u{e70f}"),
    ("sql", "\u{e706}"),
    ("sqlite", "\u{e706}"),
    ("svelte", "\u{e698}"),
    ("svg", "\u{f1c5}"),
    ("swift", "\u{e755}"),
    ("tar", "\u{f1c6}"),
    ("toml", "\u{e60b}"),
    ("ts", "\u{e628}"),
    ("ttf", "\u{f031}"),
    ("vue", "\u{e6d0}"),
    ("wav", "\u{f001}"),
    ("webp", "\u{f1c5}"),
    ("woff", "\u{f031}"),
    ("woff2", "\u{f031}"),
    ("xz", "\u{f1c6}"),
    ("yaml", "\u{e73a}"),
    ("yml", "\u{e73a}"),
    ("zig", "\u{e6a9}"),
    ("zip", "\u{f1c6}"),
    ("zsh", "\u{e795}"),
];

/// Returns the glyph for `path`'s extension (or, without one, its file name),
/// falling back to [`FALLBACK`].
pub fn file_icon(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let key = if ext.is_empty() {
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase()
    } else {
        ext
    };
    ICONS
        .binary_search_by(|(k, _)| (*k).cmp(key.as_str()))
        .map_or(FALLBACK, |i| ICONS[i].1)
}

#[cfg(test)]
#[path = "icons_test.rs"]
mod tests;
