use super::*;

fn rows(s: &str, width: usize) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut breaks = row_breaks(chars.iter().copied(), width);
    breaks.push(chars.len());
    breaks
        .windows(2)
        .map(|w| chars[w[0]..w[1]].iter().collect())
        .collect()
}

#[test]
fn fits_in_one_row() {
    assert_eq!(wrap_row_count("", 10), 1);
    assert_eq!(wrap_row_count("abcdefghij", 10), 1);
    assert_eq!(rows("short", 10), ["short"]);
}

#[test]
fn zero_width_disables_wrapping() {
    assert_eq!(wrap_row_count(&"x".repeat(100), 0), 1);
    assert_eq!(row_breaks("abc".chars(), 0), vec![0]);
}

#[test]
fn breaks_after_whitespace_instead_of_inside_a_word() {
    assert_eq!(rows("hello world foo", 8), ["hello ", "world ", "foo"]);
    assert_eq!(
        rows("such as the individual list flavors", 24),
        ["such as the individual ", "list flavors"]
    );
}

#[test]
fn a_word_longer_than_the_pane_is_broken_at_the_edge() {
    assert_eq!(rows("abcdefghijkl", 5), ["abcde", "fghij", "kl"]);
    assert_eq!(rows("ab abcdefghij", 5), ["ab ", "abcde", "fghij"]);
}

#[test]
fn hanging_whitespace_never_starts_a_row() {
    // The space after "abcde" would overflow; it hangs instead of wrapping.
    assert_eq!(rows("abcde fgh", 5), ["abcde ", "fgh"]);
    for row in rows("one two three four five six", 7) {
        assert!(!row.starts_with(' '), "row {row:?} starts with a space");
    }
}

#[test]
fn wide_characters_do_not_straddle_a_row_boundary() {
    // Each CJK char is 2 cells, so at width 5 only two fit per row (4 cells) and
    // the fifth cell is wasted. The naive `total_width.div_ceil(width)` says 3
    // rows for 14 cells; the real wrap needs 4.
    let s = "你好世界你好世"; // 7 chars, 14 cells
    assert_eq!(str_width(s), 14);
    assert_eq!(wrap_row_count(s, 5), 4);
    assert_ne!(14usize.div_ceil(5), wrap_row_count(s, 5));
}

#[test]
fn a_character_wider_than_the_pane_still_terminates() {
    assert_eq!(wrap_row_count("你你你", 1), 3);
}

#[test]
fn row_count_always_equals_the_number_of_breaks() {
    for s in [
        "",
        "a",
        "hello world",
        "日本語のテキスト and ascii mixed in a line",
        "a\u{301}bcd efg",
        "    leading spaces and    gaps",
    ] {
        for w in 1..14 {
            let n = row_breaks(s.chars(), w).len();
            assert_eq!(n.max(1), wrap_row_count(s, w).max(1), "{s:?} at width {w}");
        }
    }
}

#[test]
fn every_character_lands_in_exactly_one_row() {
    let s = "the quick brown fox jumps over the lazy dog 你好 and more";
    for w in 1..20 {
        assert_eq!(rows(s, w).concat(), s, "width {w}");
    }
}

#[test]
fn zero_width_marks_do_not_consume_cells() {
    assert_eq!(str_width("a\u{301}"), 1);
}
