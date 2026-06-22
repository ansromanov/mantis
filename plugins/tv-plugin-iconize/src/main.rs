//! Bundled iconize plugin for tree-viewer (tv).
//!
//! On `init`, sends a `set_icon_map` action with Nerd Font glyphs for ~80
//! file extensions. Requires `icons = true` in `tv.toml` and a Nerd Font
//! terminal. Exits cleanly on `shutdown`.

use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let msg: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };
        match msg["event"].as_str().unwrap_or("") {
            "init" => {
                send_icon_map(&mut io::stdout().lock());
            }
            "shutdown" => break,
            _ => {}
        }
    }
}

type IconMap = BTreeMap<&'static str, &'static str>;

fn icon_pairs() -> IconMap {
    BTreeMap::from([
        ("rs", "\u{e7a8}"),
        ("py", "\u{e73c}"),
        ("js", "\u{e74e}"),
        ("ts", "\u{e628}"),
        ("go", "\u{e627}"),
        ("java", "\u{e738}"),
        ("c", "\u{e79b}"),
        ("cpp", "\u{e61d}"),
        ("cs", "\u{e77f}"),
        ("zig", "\u{e6a9}"),
        ("rb", "\u{e739}"),
        ("php", "\u{e73d}"),
        ("lua", "\u{e620}"),
        ("hs", "\u{e61f}"),
        ("swift", "\u{e755}"),
        ("kt", "\u{e634}"),
        ("kts", "\u{e634}"),
        ("dart", "\u{e798}"),
        ("ex", "\u{e62d}"),
        ("clj", "\u{e768}"),
        ("cljs", "\u{e768}"),
        ("erl", "\u{e7b1}"),
        ("scala", "\u{e737}"),
        ("r", "\u{e71c}"),
        ("rmd", "\u{e71c}"),
        ("html", "\u{e736}"),
        ("htm", "\u{e736}"),
        ("css", "\u{e749}"),
        ("scss", "\u{e74b}"),
        ("less", "\u{e758}"),
        ("vue", "\u{e6d0}"),
        ("svelte", "\u{e698}"),
        ("json", "\u{e60b}"),
        ("yaml", "\u{e73a}"),
        ("yml", "\u{e73a}"),
        ("toml", "\u{e60b}"),
        ("sql", "\u{e706}"),
        ("db", "\u{e706}"),
        ("sqlite", "\u{e706}"),
        ("graphql", "\u{e844}"),
        ("gql", "\u{e844}"),
        ("dockerfile", "\u{e7b0}"),
        ("sh", "\u{e795}"),
        ("bash", "\u{e795}"),
        ("zsh", "\u{e795}"),
        ("fish", "\u{e795}"),
        ("md", "\u{e73e}"),
        ("markdown", "\u{e73e}"),
        ("lock", "\u{e6c6}"),
        ("exe", "\u{e70f}"),
        ("bin", "\u{e70f}"),
        ("so", "\u{e70f}"),
        ("dll", "\u{e70f}"),
        ("dylib", "\u{e70f}"),
        ("png", "\u{f1c5}"),
        ("jpg", "\u{f1c5}"),
        ("jpeg", "\u{f1c5}"),
        ("gif", "\u{f1c5}"),
        ("svg", "\u{f1c5}"),
        ("ico", "\u{f1c5}"),
        ("webp", "\u{f1c5}"),
        ("mp4", "\u{f03d}"),
        ("avi", "\u{f03d}"),
        ("mkv", "\u{f03d}"),
        ("mov", "\u{f03d}"),
        ("mp3", "\u{f001}"),
        ("wav", "\u{f001}"),
        ("ogg", "\u{f001}"),
        ("flac", "\u{f001}"),
        ("m4a", "\u{f001}"),
        ("pdf", "\u{f1c1}"),
        ("epub", "\u{f1c1}"),
        ("mobi", "\u{f1c1}"),
        ("zip", "\u{f1c6}"),
        ("tar", "\u{f1c6}"),
        ("gz", "\u{f1c6}"),
        ("xz", "\u{f1c6}"),
        ("bz2", "\u{f1c6}"),
        ("7z", "\u{f1c6}"),
        ("rar", "\u{f1c6}"),
        ("ttf", "\u{f031}"),
        ("otf", "\u{f031}"),
        ("woff", "\u{f031}"),
        ("woff2", "\u{f031}"),
        ("eot", "\u{f031}"),
        ("node", "\u{e718}"),
    ])
}

fn send_icon_map(out: &mut impl Write) {
    let mut icons = serde_json::Map::new();
    for (k, v) in icon_pairs() {
        icons.insert(k.to_string(), serde_json::Value::String(v.to_string()));
    }
    let mut params = serde_json::Map::new();
    params.insert(
        "dir_open".to_string(),
        serde_json::Value::String("\u{f07c}".to_string()),
    );
    params.insert(
        "dir_closed".to_string(),
        serde_json::Value::String("\u{f07b}".to_string()),
    );
    params.insert(
        "fallback".to_string(),
        serde_json::Value::String("\u{f15b}".to_string()),
    );
    params.insert("icons".to_string(), serde_json::Value::Object(icons));

    let msg = serde_json::json!({
        "event": "action",
        "action": "set_icon_map",
        "params": params
    });
    let _ = writeln!(out, "{}", serde_json::to_string(&msg).unwrap());
    let _ = out.flush();
}

#[cfg(test)]
#[path = "main_test.rs"]
mod tests;
