#![recursion_limit = "512"]
//! Bundled Nerd Font icon plugin for tree-viewer (tv).
//!
//! Implements the tv plugin protocol to provide per-file-extension icon glyphs
//! for the file tree. On `init`, emits a single `set_icon_map` action containing
//! directory icons, a fallback icon, and a map of file extension to Nerd Font
//! glyph. Requires a Nerd Font to be active in the terminal.
//!
//! The icon map mirrors the one previously provided by `iconize.sh`. All glyphs
//! are encoded as Rust Unicode escape sequences so the source file remains
//! ASCII-safe.
//!
//! On `shutdown` the event loop exits immediately.

use std::io::{self, BufRead, Write};

#[cfg(test)]
#[path = "main_test.rs"]
mod tests;

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_loop(stdin.lock(), stdout.lock());
}

/// Core event loop. Extracted for testability.
pub fn run_loop(input: impl BufRead, mut output: impl Write) {
    for line in input.lines() {
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
        let event = msg["event"].as_str().unwrap_or("");
        match event {
            "init" => send_icon_map(&mut output),
            "shutdown" => break,
            _ => {}
        }
    }
}

fn send_icon_map(output: &mut impl Write) {
    // Directory icons.
    let dir_open = "\u{f07c}"; // nf-fa-folder_open
    let dir_closed = "\u{f07b}"; // nf-fa-folder
    let fallback = "\u{f15b}"; // nf-fa-file_code

    // Language icons.
    let rs = "\u{e7a8}"; // nf-dev-rust
    let py = "\u{e73c}"; // nf-dev-python
    let js = "\u{e74e}"; // nf-dev-javascript
    let ts = "\u{e628}"; // nf-dev-typescript
    let go = "\u{e627}"; // nf-dev-go
    let java = "\u{e738}"; // nf-dev-java
    let c = "\u{e79b}"; // nf-dev-c
    let cpp = "\u{e61d}"; // nf-dev-cplusplus
    let cs = "\u{e77f}"; // nf-dev-csharp
    let zig = "\u{e6a9}"; // nf-dev-zig
    let rb = "\u{e739}"; // nf-dev-ruby
    let php = "\u{e73d}"; // nf-dev-php
    let lua = "\u{e620}"; // nf-dev-lua
    let hs = "\u{e61f}"; // nf-dev-haskell
    let swift = "\u{e755}"; // nf-dev-swift
    let kotlin = "\u{e634}"; // nf-dev-kotlin
    let dart = "\u{e798}"; // nf-dev-dart
    let elixir = "\u{e62d}"; // nf-dev-elixir
    let clj = "\u{e768}"; // nf-dev-clojure
    let erl = "\u{e7b1}"; // nf-dev-erlang
    let scala = "\u{e737}"; // nf-dev-scala
    let r = "\u{e71c}"; // nf-dev-r

    // Web / markup.
    let html = "\u{e736}"; // nf-dev-html5
    let css = "\u{e749}"; // nf-dev-css3
    let scss = "\u{e74b}"; // nf-dev-sass
    let less = "\u{e758}"; // nf-dev-less
    let vue = "\u{e6d0}"; // nf-dev-vue
    let svelte = "\u{e698}"; // nf-dev-svelte

    // Config / data.
    let json = "\u{e60b}"; // nf-dev-json (reused for toml)
    let yaml = "\u{e73a}"; // nf-dev-yaml
    let sql = "\u{e706}"; // nf-dev-database
    let graphql = "\u{e844}"; // nf-dev-graphql
    let docker = "\u{e7b0}"; // nf-dev-docker

    // Shell / scripts.
    let sh = "\u{e795}"; // nf-dev-terminal

    // Other.
    let md = "\u{e73e}"; // nf-dev-markdown
    let lock = "\u{e6c6}"; // nf-dev-lock
    let exe = "\u{e70f}"; // nf-dev-terminal_badge
    let img = "\u{f1c5}"; // nf-fa-image
    let vid = "\u{f03d}"; // nf-fa-video
    let aud = "\u{f001}"; // nf-fa-music
    let pdf = "\u{f1c1}"; // nf-fa-file_pdf
    let archive = "\u{f1c6}"; // nf-fa-file_archive
    let font = "\u{f031}"; // nf-fa-font
    let node = "\u{e718}"; // nf-dev-nodejs_small

    let icons = serde_json::json!({
        "rs": rs, "py": py, "js": js, "ts": ts, "go": go,
        "java": java, "c": c, "cpp": cpp, "cs": cs, "zig": zig,
        "rb": rb, "php": php, "lua": lua, "hs": hs, "swift": swift,
        "kt": kotlin, "kts": kotlin,
        "dart": dart, "ex": elixir, "clj": clj, "cljs": clj,
        "erl": erl, "scala": scala, "r": r, "rmd": r,
        "html": html, "htm": html,
        "css": css, "scss": scss, "less": less, "vue": vue, "svelte": svelte,
        "json": json, "yaml": yaml, "yml": yaml, "toml": json,
        "sql": sql, "db": sql, "sqlite": sql,
        "graphql": graphql, "gql": graphql,
        "dockerfile": docker,
        "sh": sh, "bash": sh, "zsh": sh, "fish": sh,
        "md": md, "markdown": md,
        "lock": lock,
        "exe": exe, "bin": exe, "so": exe, "dll": exe, "dylib": exe,
        "png": img, "jpg": img, "jpeg": img, "gif": img,
        "svg": img, "ico": img, "webp": img,
        "mp4": vid, "avi": vid, "mkv": vid, "mov": vid,
        "mp3": aud, "wav": aud, "ogg": aud, "flac": aud, "m4a": aud,
        "pdf": pdf, "epub": pdf, "mobi": pdf,
        "zip": archive, "tar": archive, "gz": archive, "xz": archive,
        "bz2": archive, "7z": archive, "rar": archive,
        "ttf": font, "otf": font, "woff": font, "woff2": font, "eot": font,
        "node": node
    });

    let msg = serde_json::json!({
        "event": "action",
        "action": "set_icon_map",
        "params": {
            "dir_open": dir_open,
            "dir_closed": dir_closed,
            "fallback": fallback,
            "icons": icons
        }
    });
    let _ = writeln!(
        output,
        "{}",
        serde_json::to_string(&msg).unwrap_or_default()
    );
    let _ = output.flush();
}
