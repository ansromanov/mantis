//! Brace-delimited detectors: C-like, JSON, shell and HCL.

use crate::fold::FoldRegion;

/// Detects foldable regions in brace-delimited languages (Rust, Go, C, Java,
/// JS, …).
///
/// Walks `text` character by character, maintaining a nesting stack for `{…}`
/// pairs.  Braces inside the following contexts are ignored:
///
/// * Line comments (`// …`)
/// * Block comments (`/* … */`)
/// * Double-quoted strings (`"…"` with `\"` escapes)
/// * Rust raw strings (`r"…"`, `r#"…"#`, `r##"…"##`, …)
/// * Go backtick strings (`` `…` ``)
///
/// Returns one region per `{…}` block that spans more than one line.  The
/// nesting stack is `Vec<usize>` (line number), so deeply nested files are
/// bounded only by available memory.
pub fn brace_fold(text: &str) -> Vec<FoldRegion> {
    brace_fold_impl(text, false)
}

/// Like [`brace_fold`], but also tracks `[…]` bracket blocks as foldable
/// regions — for JSON, a multiline array is as foldable as an object.
/// Braces and brackets share a single line-position stack (no type
/// checking), matching `brace_fold`'s existing tolerance of unbalanced
/// input: a `]` closes whatever is on top of the stack, `{` or `[` alike.
pub fn brace_fold_with_brackets(text: &str) -> Vec<FoldRegion> {
    brace_fold_impl(text, true)
}

fn brace_fold_impl(text: &str, track_brackets: bool) -> Vec<FoldRegion> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return Vec::new();
    }

    #[derive(Clone, Copy)]
    enum St {
        Normal,
        LineCmt,
        BlockCmt,
        DqStr,
        SqStr,
        RawStr(usize),
        BtStr,
    }

    let mut st = St::Normal;
    let mut line = 0usize;
    let mut stack: Vec<usize> = Vec::new();
    let mut regions: Vec<FoldRegion> = Vec::new();
    let mut i = 0;

    while i < len {
        let b = bytes[i];
        match st {
            St::Normal => match b {
                b'\n' => line += 1,
                b'{' => stack.push(line),
                b'}' => {
                    if let Some(start) = stack.pop() {
                        if line > start {
                            regions.push(FoldRegion { start, end: line });
                        }
                    }
                }
                b'[' if track_brackets => stack.push(line),
                b']' if track_brackets => {
                    if let Some(start) = stack.pop() {
                        if line > start {
                            regions.push(FoldRegion { start, end: line });
                        }
                    }
                }
                b'/' if i + 1 < len => match bytes[i + 1] {
                    b'/' => {
                        st = St::LineCmt;
                        i += 1;
                    }
                    b'*' => {
                        st = St::BlockCmt;
                        i += 1;
                    }
                    _ => {}
                },
                b'"' => st = St::DqStr,
                b'\'' => st = St::SqStr,
                b'`' => st = St::BtStr,
                b'r' => {
                    let prev_ident =
                        i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
                    if !prev_ident {
                        let mut j = i + 1;
                        let mut hashes = 0usize;
                        while j < len && bytes[j] == b'#' {
                            hashes += 1;
                            j += 1;
                        }
                        if j < len && bytes[j] == b'"' {
                            st = St::RawStr(hashes);
                            i = j;
                        }
                    }
                }
                _ => {}
            },
            St::LineCmt => {
                if b == b'\n' {
                    st = St::Normal;
                    line += 1;
                }
            }
            St::BlockCmt => {
                if b == b'\n' {
                    line += 1;
                } else if b == b'*' && i + 1 < len && bytes[i + 1] == b'/' {
                    st = St::Normal;
                    i += 1;
                }
            }
            St::DqStr => {
                if b == b'\\' && i + 1 < len {
                    if bytes[i + 1] == b'\n' {
                        line += 1;
                    }
                    i += 1;
                } else if b == b'"' {
                    st = St::Normal;
                } else if b == b'\n' {
                    line += 1;
                }
            }
            St::SqStr => {
                if b == b'\\' {
                    i += 1;
                } else if b == b'\'' {
                    st = St::Normal;
                }
            }
            St::RawStr(hashes) => {
                if b == b'\n' {
                    line += 1;
                } else if b == b'"' {
                    let mut j = i + 1;
                    let mut seen = 0usize;
                    while j < len && bytes[j] == b'#' && seen < hashes {
                        seen += 1;
                        j += 1;
                    }
                    if seen == hashes {
                        st = St::Normal;
                        i = j.wrapping_sub(1);
                    }
                }
            }
            St::BtStr => {
                if b == b'`' {
                    st = St::Normal;
                } else if b == b'\n' {
                    line += 1;
                }
            }
        }
        i += 1;
    }

    regions
}

// ---------------------------------------------------------------------------
// Shell brace-nesting detector
// ---------------------------------------------------------------------------

/// Detects foldable regions in shell scripts (sh, bash, zsh).
///
/// Like [`brace_fold`], walks `text` character by character maintaining a
/// nesting stack for `{…}` pairs, but uses shell-specific syntax rules:
///
/// * Line comments (`# …` at word start only — `$#`/`${#v}` are expansions)
/// * Single-quoted strings (`'…'` — no escape processing)
/// * Double-quoted strings (`"…"` with `\"` escapes)
/// * Backslash escapes outside strings (`\'` does not open a string)
/// * Heredocs (`<<WORD`, `<<'WORD'`, `<<-WORD` … `WORD` — braces inside are inert)
/// * Arithmetic contexts (`(( … ))` — `<<` inside is a shift, not a heredoc)
///
/// Returns one region per `{…}` block that spans more than one line.
pub fn shell_brace_fold(text: &str) -> Vec<FoldRegion> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return Vec::new();
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum St {
        Normal,
        LineCmt,
        SqStr,
        DqStr,
        Heredoc,
    }

    let mut st = St::Normal;
    let mut line = 0usize;
    let mut stack: Vec<usize> = Vec::new();
    let mut regions: Vec<FoldRegion> = Vec::new();
    let mut heredoc_delim: Vec<u8> = Vec::new();
    // Only <<- heredocs allow (tab) indentation before the closing delimiter.
    let mut heredoc_allow_indent = false;
    // Depth of `(( … ))` arithmetic contexts, where << is a shift operator.
    let mut arith_depth = 0usize;
    let mut i = 0;

    while i < len {
        let b = bytes[i];
        match st {
            St::Normal => match b {
                b'\n' => line += 1,
                b'\\' if i + 1 < len => {
                    // Escape outside strings: \' and \" must not open a string.
                    if bytes[i + 1] == b'\n' {
                        line += 1;
                    }
                    i += 1;
                }
                b'{' => stack.push(line),
                b'}' => {
                    if let Some(start) = stack.pop() {
                        if line > start {
                            regions.push(FoldRegion { start, end: line });
                        }
                    }
                }
                // `#` starts a comment only at the start of a word; `$#` and
                // `${#var}` are parameter expansions.
                b'#' if i == 0
                    || matches!(
                        bytes[i - 1],
                        b'\n' | b' ' | b'\t' | b';' | b'&' | b'|' | b'('
                    ) =>
                {
                    st = St::LineCmt
                }
                b'\'' => st = St::SqStr,
                b'"' => st = St::DqStr,
                b'(' if i + 1 < len && bytes[i + 1] == b'(' => {
                    arith_depth += 1;
                    i += 1;
                }
                b')' if arith_depth > 0 && i + 1 < len && bytes[i + 1] == b')' => {
                    arith_depth -= 1;
                    i += 1;
                }
                b'<' if i + 1 < len && bytes[i + 1] == b'<' => {
                    if arith_depth > 0 {
                        // Shift operator inside (( … )) — not a heredoc.
                        i += 1;
                    } else if i + 2 < len && bytes[i + 2] == b'<' {
                        // Skip <<< (here-string) — not a heredoc.
                        i += 2;
                    } else {
                        i += 1;
                        let dash = i + 1 < len && bytes[i + 1] == b'-';
                        if dash {
                            i += 1;
                        }
                        i += 1;
                        while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                            i += 1;
                        }
                        let word_start = i;
                        while i < len && bytes[i] != b'\n' && bytes[i] != b' ' && bytes[i] != b'\t'
                        {
                            i += 1;
                        }
                        // Strip quoting from the delimiter word: <<'EOF',
                        // <<"EOF" and <<\EOF all terminate on a bare EOF line.
                        let mut word = &bytes[word_start..i];
                        if word.first() == Some(&b'\\') {
                            word = &word[1..];
                        } else if word.len() >= 2
                            && (word[0] == b'\'' || word[0] == b'"')
                            && word[word.len() - 1] == word[0]
                        {
                            word = &word[1..word.len() - 1];
                        }
                        if !word.is_empty() {
                            heredoc_delim.clear();
                            heredoc_delim.extend_from_slice(word);
                            heredoc_allow_indent = dash;
                            st = St::Heredoc;
                            continue;
                        }
                    }
                }
                _ => {}
            },
            St::LineCmt => {
                if b == b'\n' {
                    st = St::Normal;
                    line += 1;
                }
            }
            St::SqStr => {
                if b == b'\'' {
                    st = St::Normal;
                } else if b == b'\n' {
                    line += 1;
                }
            }
            St::DqStr => {
                if b == b'\\' && i + 1 < len {
                    if bytes[i + 1] == b'\n' {
                        line += 1;
                    }
                    i += 1;
                } else if b == b'"' {
                    st = St::Normal;
                } else if b == b'\n' {
                    line += 1;
                }
            }
            St::Heredoc => {
                if b == b'\n' {
                    line += 1;
                    let after_nl = i + 1;
                    let remaining = len - after_nl;
                    if remaining >= heredoc_delim.len() {
                        // Only <<- strips leading tabs before the delimiter;
                        // a plain << delimiter must start the line.
                        let mut start = after_nl;
                        if heredoc_allow_indent {
                            while start < len && bytes[start] == b'\t' {
                                start += 1;
                            }
                        }
                        let avail = len - start;
                        if avail >= heredoc_delim.len() {
                            let mut matches = true;
                            for (j, &d) in heredoc_delim.iter().enumerate() {
                                if bytes[start + j] != d {
                                    matches = false;
                                    break;
                                }
                            }
                            if matches {
                                let after_delim = start + heredoc_delim.len();
                                if after_delim >= len
                                    || bytes[after_delim] == b'\n'
                                    || bytes[after_delim] == b'\r'
                                {
                                    st = St::Normal;
                                }
                            }
                        }
                    }
                }
            }
        }
        i += 1;
    }

    regions
}

// ---------------------------------------------------------------------------
// HCL brace-nesting detector
// ---------------------------------------------------------------------------

/// Detects foldable regions in HashiCorp Configuration Language (HCL) files —
/// Terraform, TFLint, OpenBao, Nomad, Consul.
///
/// Like [`brace_fold`], walks `text` character by character maintaining a
/// nesting stack for `{…}` pairs, but uses HCL-specific syntax rules:
///
/// * Line comments (`# …` and `// …`)
/// * Block comments (`/* … */`)
/// * Double-quoted strings (`"…"` with `\"` escapes)
/// * Heredocs (`<<WORD`, `<<'WORD'`, `<<"WORD"`, `<<-WORD` … `WORD` — braces
///   inside are inert)
///
/// Returns one region per `{…}` block that spans more than one line.
pub fn hcl_brace_fold(text: &str) -> Vec<FoldRegion> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return Vec::new();
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum St {
        Normal,
        LineCmt,
        BlockCmt,
        DqStr,
        Heredoc,
    }

    let mut st = St::Normal;
    let mut line = 0usize;
    let mut stack: Vec<usize> = Vec::new();
    let mut regions: Vec<FoldRegion> = Vec::new();
    let mut heredoc_delim: Vec<u8> = Vec::new();
    // Only <<- heredocs allow (tab) indentation before the closing delimiter.
    let mut heredoc_allow_indent = false;
    let mut i = 0;

    while i < len {
        let b = bytes[i];
        match st {
            St::Normal => match b {
                b'\n' => line += 1,
                b'{' => stack.push(line),
                b'}' => {
                    if let Some(start) = stack.pop() {
                        if line > start {
                            regions.push(FoldRegion { start, end: line });
                        }
                    }
                }
                // `#` and `//` start a comment that runs to end of line.
                b'#' => st = St::LineCmt,
                b'/' if i + 1 < len && bytes[i + 1] == b'/' => {
                    st = St::LineCmt;
                    i += 1;
                }
                b'/' if i + 1 < len && bytes[i + 1] == b'*' => {
                    st = St::BlockCmt;
                    i += 1;
                }
                b'"' => st = St::DqStr,
                b'<' if i + 1 < len && bytes[i + 1] == b'<' => {
                    i += 1;
                    let dash = i + 1 < len && bytes[i + 1] == b'-';
                    if dash {
                        i += 1;
                    }
                    i += 1;
                    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                        i += 1;
                    }
                    let word_start = i;
                    while i < len && bytes[i] != b'\n' && bytes[i] != b' ' && bytes[i] != b'\t' {
                        i += 1;
                    }
                    // Strip quoting from the delimiter word: <<'EOF' and
                    // <<"EOF" both terminate on a bare EOF line.
                    let mut word = &bytes[word_start..i];
                    if word.len() >= 2
                        && (word[0] == b'\'' || word[0] == b'"')
                        && word[word.len() - 1] == word[0]
                    {
                        word = &word[1..word.len() - 1];
                    }
                    if !word.is_empty() {
                        heredoc_delim.clear();
                        heredoc_delim.extend_from_slice(word);
                        heredoc_allow_indent = dash;
                        st = St::Heredoc;
                        continue;
                    }
                }
                _ => {}
            },
            St::LineCmt => {
                if b == b'\n' {
                    st = St::Normal;
                    line += 1;
                }
            }
            St::BlockCmt => {
                if b == b'\n' {
                    line += 1;
                } else if b == b'*' && i + 1 < len && bytes[i + 1] == b'/' {
                    st = St::Normal;
                    i += 1;
                }
            }
            St::DqStr => {
                if b == b'\\' && i + 1 < len {
                    if bytes[i + 1] == b'\n' {
                        line += 1;
                    }
                    i += 1;
                } else if b == b'"' {
                    st = St::Normal;
                } else if b == b'\n' {
                    line += 1;
                }
            }
            St::Heredoc => {
                if b == b'\n' {
                    line += 1;
                    let after_nl = i + 1;
                    let remaining = len - after_nl;
                    if remaining >= heredoc_delim.len() {
                        // Only <<- strips leading whitespace before the
                        // delimiter; a plain << delimiter must start the line.
                        let mut start = after_nl;
                        if heredoc_allow_indent {
                            while start < len && (bytes[start] == b'\t' || bytes[start] == b' ') {
                                start += 1;
                            }
                        }
                        let avail = len - start;
                        if avail >= heredoc_delim.len() {
                            let mut matches = true;
                            for (j, &d) in heredoc_delim.iter().enumerate() {
                                if bytes[start + j] != d {
                                    matches = false;
                                    break;
                                }
                            }
                            if matches {
                                let after_delim = start + heredoc_delim.len();
                                if after_delim >= len
                                    || bytes[after_delim] == b'\n'
                                    || bytes[after_delim] == b'\r'
                                {
                                    st = St::Normal;
                                }
                            }
                        }
                    }
                }
            }
        }
        i += 1;
    }

    regions
}

// ---------------------------------------------------------------------------
// Indentation-based detector (Python)
// ---------------------------------------------------------------------------
