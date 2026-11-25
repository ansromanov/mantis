//! Template (`{{ }}`/`{% %}`) block detector.

use crate::fold::FoldRegion;

/// Detects structural blocks in Go-template and Jinja2 template files.
///
/// Matching directives are `define`/`if`/`range`/`with` and `block`/`if`/
/// `for`/`macro`, respectively. Template comments are removed from the scan,
/// including multiline comments, so directive-looking text inside comments
/// cannot create or close a region. Regions are returned in closing order,
/// matching the existing brace detector convention.
pub fn template_fold(text: &str) -> Vec<FoldRegion> {
    let mut stack: Vec<(String, usize)> = Vec::new();
    let mut regions = Vec::new();
    let mut i = 0;
    let mut line = 0;
    let bytes = text.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            line += 1;
            i += 1;
            continue;
        }
        if text.get(i..).is_some_and(|rest| rest.starts_with("{{/*")) {
            if let Some(end) = text[i + 4..].find("*/}}") {
                line += text[i + 4..i + 4 + end]
                    .bytes()
                    .filter(|&b| b == b'\n')
                    .count();
                i += end + 8;
                continue;
            }
            break;
        }
        let (open, close) = if text.get(i..).is_some_and(|rest| rest.starts_with("{{")) {
            ("{{", "}}")
        } else if text.get(i..).is_some_and(|rest| rest.starts_with("{%")) {
            ("{%", "%}")
        } else if text.get(i..).is_some_and(|rest| rest.starts_with("{#")) {
            if let Some(end) = text[i + 2..].find("#}") {
                line += text[i + 2..i + 2 + end]
                    .bytes()
                    .filter(|&b| b == b'\n')
                    .count();
                i += end + 4;
                continue;
            }
            break;
        } else {
            i += 1;
            continue;
        };
        let Some(end) = text[i + 2..].find(close) else {
            break;
        };
        let content = text[i + 2..i + 2 + end].trim_matches('-').trim();
        let first = content.split_whitespace().next().unwrap_or("");
        if content.starts_with("/*") {
            line += content.bytes().filter(|&b| b == b'\n').count();
        } else if is_template_open(first, open) {
            stack.push((first.to_string(), line));
        } else if is_template_close(first) {
            if let Some((_, start)) = stack.pop() {
                if line > start {
                    regions.push(FoldRegion { start, end: line });
                }
            }
        }
        let consumed = &text[i..i + 2 + end + 2];
        line += consumed.bytes().filter(|&b| b == b'\n').count();
        i += 2 + end + 2;
    }
    regions
}

fn is_template_open(word: &str, delimiter: &str) -> bool {
    (delimiter == "{{" && matches!(word, "define" | "block" | "if" | "range" | "with"))
        || (delimiter == "{%" && matches!(word, "block" | "if" | "for" | "macro"))
}

fn is_template_close(word: &str) -> bool {
    matches!(word, "end" | "endblock" | "endif" | "endfor" | "endmacro")
}
