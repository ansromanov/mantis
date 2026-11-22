//! Picker: in file.

use crate::list_picker::ListPicker;

/// A single match occurrence within a file for in-file search.
#[derive(Clone, Debug)]
pub struct InFileMatch {
    pub line: usize,
    pub col: usize,
    pub len: usize,
}

/// State for in-file search (/ while content panel is focused).
pub struct InFileSearch {
    pub query: String,
    pub matches: Vec<InFileMatch>,
    pub current: usize,
    /// Interpret the query as a regular expression (Ctrl+R).
    pub regex: bool,
    /// Match case-sensitively instead of the default insensitive (Ctrl+A).
    pub case_sensitive: bool,
    /// Match whole words only (Ctrl+W).
    pub whole_word: bool,
}

impl Default for InFileSearch {
    fn default() -> Self {
        Self::new()
    }
}

impl InFileSearch {
    pub fn new() -> Self {
        InFileSearch {
            query: String::new(),
            matches: Vec::new(),
            current: 0,
            regex: false,
            case_sensitive: false,
            whole_word: false,
        }
    }

    pub fn push(&mut self, c: char) {
        self.query.push(c);
    }

    pub fn pop(&mut self) {
        self.query.pop();
    }

    /// Re-computes matches by calling `get_line(i)` for each 0-based line index
    /// up to `line_count`. The caller provides the line provider (which may read
    /// from a virtual file or from an in-memory vec).
    pub fn refresh(&mut self, line_count: usize, get_line: impl Fn(usize) -> Option<String>) {
        self.matches.clear();
        self.current = 0;
        if self.query.is_empty() {
            return;
        }

        if self.regex || self.whole_word {
            let Some(re) = crate::search::build_search_regex(
                &self.query,
                self.regex,
                self.whole_word,
                self.case_sensitive,
            ) else {
                return;
            };
            for i in 0..line_count {
                let Some(line) = get_line(i) else { continue };
                for mat in re.find_iter(&line) {
                    // A pattern like `x*` matches the empty string at every
                    // position; zero-length matches are useless as highlights.
                    if mat.start() == mat.end() {
                        continue;
                    }
                    let char_start = line[..mat.start()].chars().count();
                    let char_len = line[mat.start()..mat.end()].chars().count();
                    self.matches.push(InFileMatch {
                        line: i,
                        col: char_start,
                        len: char_len,
                    });
                }
            }
        } else {
            let q = if self.case_sensitive {
                self.query.clone()
            } else {
                self.query.to_lowercase()
            };
            let q_chars: Vec<char> = q.chars().collect();
            let q_char_len = q_chars.len();
            if q_char_len == 0 {
                return;
            }
            for i in 0..line_count {
                let Some(line) = get_line(i) else { continue };
                let target = if self.case_sensitive {
                    line
                } else {
                    line.to_lowercase()
                };
                let target_chars: Vec<char> = target.chars().collect();
                if target_chars.len() < q_char_len {
                    continue;
                }
                for start in 0..=target_chars.len() - q_char_len {
                    if target_chars[start..start + q_char_len] == q_chars[..] {
                        self.matches.push(InFileMatch {
                            line: i,
                            col: start,
                            len: q_char_len,
                        });
                    }
                }
            }
        }
    }
}

impl ListPicker for InFileSearch {
    fn query_push(&mut self, c: char) {
        self.push(c);
    }
    fn query_pop(&mut self) {
        self.pop();
    }
    fn query_is_empty(&self) -> bool {
        self.query.is_empty()
    }
    fn results_len(&self) -> usize {
        self.matches.len()
    }
    fn selected(&self) -> usize {
        self.current
    }
    fn set_selected(&mut self, i: usize) {
        self.current = i;
    }
}
