//! Lightweight Rust declaration detection for language-provider plugins.
//!
//! This detector recognizes common type, module, implementation, and function
//! declarations with source ranges and parent names. It deliberately remains a
//! heuristic: the provider protocol accepts symbols from parsers with richer
//! language support, while bundled plugins can share this small implementation.

use crate::plugin::types::Symbol;

/// Finds common Rust declarations and their enclosing brace-delimited ranges.
pub fn rust_symbols(source: &str) -> Vec<Symbol> {
    let lines: Vec<&str> = source.lines().collect();
    let mut symbols = Vec::new();

    for (line_index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let Some((mut kind, name)) = rust_declaration(trimmed) else {
            continue;
        };
        let parent_symbol = symbols
            .iter()
            .filter(|symbol: &&Symbol| {
                symbol.line < line_index && symbol.end_line.is_some_and(|end| end >= line_index)
            })
            .filter(|symbol| {
                matches!(
                    symbol.kind.as_str(),
                    "impl" | "struct" | "enum" | "trait" | "module"
                )
            })
            .min_by_key(|symbol| symbol.end_line.unwrap_or(symbol.line) - symbol.line)
            .cloned();
        if kind == "function"
            && parent_symbol
                .as_ref()
                .is_some_and(|symbol| symbol.kind == "impl")
        {
            kind = "method".to_string();
        }
        let parent = parent_symbol.map(|symbol| symbol.name);
        let end_line = rust_block_end(&lines, line_index);
        symbols.push(Symbol {
            name,
            kind,
            line: line_index,
            end_line: Some(end_line),
            parent,
        });
    }
    symbols
}

fn rust_declaration(line: &str) -> Option<(String, String)> {
    for (token, kind) in [
        ("fn ", "function"),
        ("struct ", "struct"),
        ("enum ", "enum"),
        ("trait ", "trait"),
        ("impl ", "impl"),
        ("mod ", "module"),
    ] {
        let Some(start) = line.find(token) else {
            continue;
        };
        if !line[..start].split_whitespace().all(|prefix| {
            matches!(prefix, "pub" | "async" | "unsafe" | "const" | "default")
                || prefix.starts_with("pub(")
        }) {
            continue;
        }
        let tail = &line[start + token.len()..];
        let name = if kind == "impl" {
            let target = tail.split('{').next()?.trim();
            target
                .split_whitespace()
                .last()
                .unwrap_or(target)
                .trim_matches(|c: char| !c.is_alphanumeric() && c != '_')
        } else {
            tail.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .next()
                .unwrap_or("")
        };
        if name.is_empty() {
            continue;
        }
        return Some((kind.to_string(), name.to_string()));
    }
    None
}

fn rust_block_end(lines: &[&str], start: usize) -> usize {
    let mut depth = 0isize;
    let mut opened = false;
    for (line_index, line) in lines.iter().enumerate().skip(start) {
        for ch in line.chars() {
            match ch {
                '{' => {
                    depth += 1;
                    opened = true;
                }
                '}' if opened => depth -= 1,
                _ => {}
            }
        }
        if opened && depth <= 0 {
            return line_index;
        }
        if !opened && line.contains(';') {
            return line_index;
        }
    }
    lines.len().saturating_sub(1).max(start)
}

#[cfg(test)]
#[path = "symbols_test.rs"]
mod tests;
