//! SQL statement and block detector.

use crate::fold::FoldRegion;

/// Detects foldable SQL statements and procedural blocks.
///
/// Multiline statements beginning with a common SQL command are folded through
/// their terminating semicolon. `BEGIN`/`CASE`/`IF`/`LOOP` blocks are folded by
/// matching their corresponding `END` keyword. SQL comments and quoted
/// literals are skipped, so keywords and semicolons in them are inert.
pub fn sql_fold(text: &str) -> Vec<FoldRegion> {
    let tokens = sql_tokens(text);
    let mut regions = Vec::new();
    let mut blocks: Vec<(SqlBlock, usize)> = Vec::new();
    let mut statement_start = None;
    let mut statement_is_foldable = false;

    for (index, token) in tokens.iter().enumerate() {
        if statement_start.is_none() {
            statement_start = Some(token.line);
            statement_is_foldable = is_sql_statement_start(&token.word);
        }
        if matches!(token.word.as_str(), "IF" | "LOOP" | "CASE")
            && tokens
                .get(index.wrapping_sub(1))
                .is_some_and(|previous| previous.word == "END" && previous.line == token.line)
        {
            continue;
        }
        match token.word.as_str() {
            "BEGIN" => blocks.push((SqlBlock::Begin, token.line)),
            "CASE" => blocks.push((SqlBlock::Case, token.line)),
            "IF" => blocks.push((SqlBlock::If, token.line)),
            "LOOP" => blocks.push((SqlBlock::Loop, token.line)),
            "END" => {
                let next = tokens.get(index + 1).filter(|next| next.line == token.line);
                let expected = match next {
                    Some(next) if next.word == "IF" => SqlBlock::If,
                    Some(next) if next.word == "LOOP" => SqlBlock::Loop,
                    Some(next) if next.word == "CASE" => SqlBlock::Case,
                    _ => SqlBlock::Begin,
                };
                let position = blocks
                    .iter()
                    .rposition(|(block, _)| *block == expected)
                    .or_else(|| {
                        blocks
                            .iter()
                            .rposition(|(block, _)| *block == SqlBlock::Case)
                    });
                if let Some(position) = position {
                    let (_, start) = blocks.remove(position);
                    if token.line > start {
                        regions.push(FoldRegion {
                            start,
                            end: token.line,
                        });
                    }
                    if expected == SqlBlock::Begin
                        && blocks.last().map(|(block, _)| *block) == Some(SqlBlock::If)
                    {
                        if let Some((_, start)) = blocks.pop() {
                            if token.line > start {
                                regions.push(FoldRegion {
                                    start,
                                    end: token.line,
                                });
                            }
                        }
                    }
                }
            }
            ";" => {
                if !blocks.is_empty() {
                    continue;
                }
                if let Some(start) = statement_start.take() {
                    if statement_is_foldable && token.line > start {
                        regions.push(FoldRegion {
                            start,
                            end: token.line,
                        });
                    }
                }
                statement_is_foldable = false;
            }
            _ => {}
        }
    }
    regions
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SqlBlock {
    Begin,
    Case,
    If,
    Loop,
}

struct SqlToken {
    word: String,
    line: usize,
}

fn is_sql_statement_start(word: &str) -> bool {
    matches!(
        word,
        "ALTER"
            | "CREATE"
            | "DELETE"
            | "DECLARE"
            | "DROP"
            | "EXPLAIN"
            | "GRANT"
            | "INSERT"
            | "MERGE"
            | "REVOKE"
            | "SELECT"
            | "UPDATE"
            | "VALUES"
            | "WITH"
    )
}

fn sql_tokens(text: &str) -> Vec<SqlToken> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut line = 0;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => {
                line += 1;
                i += 1;
            }
            b' ' | b'\t' | b'\r' => i += 1,
            b'-' if bytes.get(i + 1) == Some(&b'-') => {
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i < bytes.len() {
                    if bytes[i] == b'\n' {
                        line += 1;
                    }
                    if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
            }
            b'\'' | b'"' => {
                let quote = bytes[i];
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\n' {
                        line += 1;
                    }
                    if bytes[i] == quote {
                        if bytes.get(i + 1) == Some(&quote) {
                            i += 2;
                            continue;
                        }
                        i += 1;
                        break;
                    }
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            b';' => {
                tokens.push(SqlToken {
                    word: ";".to_string(),
                    line,
                });
                i += 1;
            }
            byte if byte.is_ascii_alphabetic() || byte == b'_' => {
                let start = i;
                i += 1;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                tokens.push(SqlToken {
                    word: text[start..i].to_ascii_uppercase(),
                    line,
                });
            }
            _ => i += 1,
        }
    }
    tokens
}
