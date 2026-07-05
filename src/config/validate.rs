//! Schema validation for the loaded config.
//!
//! Deserialization with `#[serde(default)]` silently drops unrecognized keys, so
//! a typo like `qiut` under `[keys]` would take effect as nothing, with no
//! feedback to the user. To catch that, `validate_keys` re-parses the raw TOML
//! and walks it against a schema table derived from a fully-populated `Config`,
//! flagging every unknown key by its full dotted path (e.g. `keys.qiut`,
//! `theme.acent`) and attaching a nearest-match suggestion when one is close
//! enough. The loader calls this after a successful parse and surfaces the
//! warnings without failing the launch. Validation is best-effort: unparseable
//! input is left to the caller's error path.

use super::{Config, StatusBarConfig};
use crate::theme::ThemeConfig;

/// Deprecated keys accepted without warning, by full dotted path. Either folded
/// into their new home by a `migrate_legacy_*` step, or genuinely gone and safe
/// to drop silently.
const DEPRECATED_KEYS: &[&str] = &[
    // git (#342)
    "git_status",
    "git_show_deleted",
    "git_show_untracked",
    "git_show_ignored",
    "ignore_gitignore",
    "diff_mode",
    // tree/content/search
    "show_hidden",
    "tree_width",
    "tree_independent_scroll",
    "indent_guides",
    "icons",
    "word_wrap",
    "line_numbers",
    "scrollbar",
    "scroll_percentage",
    "watch",
    "show_file_info",
    "in_file_search",
    "search_context_lines",
    "keep_search_query",
    // runtime view-state, never a config value (#553)
    "git_mode",
    "git_mode_flat",
    // [keys] actions renamed or removed by the keymap refactor (#553). The
    // renamed ones are folded by `Keymap::migrate_legacy_keys`; the removed
    // ones have no current equivalent and are just dropped.
    "keys.yaml_fold_toggle",
    "keys.visual_line_blame",
    "keys.toggle_raw_markdown",
    "keys.visual_line_toggle",
];

/// Validates the raw TOML against the config schema, returning a message for
/// every unrecognized key (with a nearest-match suggestion where one is close
/// enough). Keys are reported by full path, e.g. `keys.qiut` or `theme.acent`.
/// Returns an empty list for a fully valid config.
pub(super) fn validate_keys(src: &str) -> Vec<String> {
    let Ok(actual) = src.parse::<toml::Table>() else {
        return Vec::new(); // unparseable input is handled by the caller's error path
    };
    let mut out = Vec::new();
    collect_unknown(&actual, &schema_table(), "", &mut out);
    out
}

/// Builds the set of recognized keys, keyed by table, by serializing a fully
/// populated `Config`. The theme and statusbar must be populated explicitly
/// because their default fields are all `None`, which TOML omits.
fn schema_table() -> toml::Table {
    let cfg = Config {
        theme: ThemeConfig::schema(),
        statusbar: StatusBarConfig::schema(),
        ..Config::default()
    };
    toml::Table::try_from(cfg).unwrap_or_default()
}

/// Walks `actual` against `schema`, recording any key absent from the schema.
/// Recurses into nested tables (`[keys]`, `[theme]`) so typos there are caught
/// with their full path.
fn collect_unknown(
    actual: &toml::Table,
    schema: &toml::Table,
    prefix: &str,
    out: &mut Vec<String>,
) {
    for (key, val) in actual {
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        match schema.get(key) {
            None => {
                // Skip known-deprecated keys (moved, renamed, or removed).
                if DEPRECATED_KEYS.contains(&path.as_str()) {
                    continue;
                }
                let names: Vec<&str> = schema.keys().map(String::as_str).collect();
                let hint = nearest_match(key, &names)
                    .map(|m| format!(" (did you mean '{m}'?)"))
                    .unwrap_or_default();
                out.push(format!("unknown key '{path}'{hint}"));
            }
            Some(schema_val) => {
                // The [plugins] table has user-defined plugin names as keys;
                // do not recurse into it or every plugin entry would be flagged.
                if key != "plugins" {
                    if let (Some(a), Some(s)) = (val.as_table(), schema_val.as_table()) {
                        collect_unknown(a, s, &path, out);
                    }
                }
            }
        }
    }
}

/// Returns the candidate closest to `input` by edit distance, if one is within
/// a small threshold — close enough to be a plausible typo rather than noise.
fn nearest_match(input: &str, candidates: &[&str]) -> Option<String> {
    candidates
        .iter()
        .map(|c| (levenshtein(input, c), *c))
        .filter(|(d, _)| *d <= 3)
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c.to_string())
}

/// Standard Levenshtein edit distance over Unicode scalar values.
fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0; b.len() + 1];
    for (i, ca) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

#[cfg(test)]
#[path = "validate_test.rs"]
mod tests;
