use super::*;

fn lines(s: &[&str]) -> Vec<String> {
    s.iter().map(|l| l.to_string()).collect()
}

#[test]
fn empty_input_yields_no_rows() {
    assert!(parse_side_by_side(&[]).is_empty());
}

#[test]
fn placeholder_message_yields_no_rows() {
    assert!(parse_side_by_side(&lines(&["(no diff available)"])).is_empty());
}

#[test]
fn context_line_appears_on_both_sides() {
    let rows = parse_side_by_side(&lines(&["@@ -1,1 +1,1 @@", " unchanged"]));
    assert_eq!(rows.len(), 2);
    assert!(matches!(rows[0], DiffRow::Header(_)));
    match &rows[1] {
        DiffRow::Split { left, right } => {
            assert_eq!(left.kind, CellKind::Context);
            assert_eq!(right.kind, CellKind::Context);
            assert_eq!(left.text, "unchanged");
            assert_eq!(right.text, "unchanged");
            assert_eq!(left.line_no, Some(1));
            assert_eq!(right.line_no, Some(1));
        }
        _ => panic!("expected split row"),
    }
}

#[test]
fn paired_change_aligns_old_and_new() {
    let rows = parse_side_by_side(&lines(&["@@ -1,1 +1,1 @@", "-let x = 1;", "+let x = 2;"]));
    assert_eq!(rows.len(), 2);
    match &rows[1] {
        DiffRow::Split { left, right } => {
            assert_eq!(left.kind, CellKind::Removed);
            assert_eq!(right.kind, CellKind::Added);
            assert_eq!(left.text, "let x = 1;");
            assert_eq!(right.text, "let x = 2;");
            let old_seg: String = left
                .emphasis
                .iter()
                .flat_map(|&(s, e)| left.text.chars().skip(s).take(e - s))
                .collect();
            let new_seg: String = right
                .emphasis
                .iter()
                .flat_map(|&(s, e)| right.text.chars().skip(s).take(e - s))
                .collect();
            assert_eq!(old_seg, "1");
            assert_eq!(new_seg, "2");
        }
        _ => panic!("expected split row"),
    }
}

#[test]
fn extra_addition_pads_left_with_empty() {
    let rows = parse_side_by_side(&lines(&["@@ -1,1 +1,2 @@", "-old", "+new", "+extra"]));
    assert_eq!(rows.len(), 3);
    match &rows[2] {
        DiffRow::Split { left, right } => {
            assert_eq!(left.kind, CellKind::Empty);
            assert_eq!(left.line_no, None);
            assert_eq!(right.kind, CellKind::Added);
            assert_eq!(right.text, "extra");
            assert_eq!(right.line_no, Some(2));
        }
        _ => panic!("expected split row"),
    }
}

#[test]
fn extra_removal_pads_right_with_empty() {
    let rows = parse_side_by_side(&lines(&["@@ -1,2 +1,1 @@", "-a", "-b", "+c"]));
    assert_eq!(rows.len(), 3);
    match &rows[2] {
        DiffRow::Split { left, right } => {
            assert_eq!(left.kind, CellKind::Removed);
            assert_eq!(left.text, "b");
            assert_eq!(right.kind, CellKind::Empty);
        }
        _ => panic!("expected split row"),
    }
}

#[test]
fn line_numbers_track_across_hunks() {
    let rows = parse_side_by_side(&lines(&["@@ -10,2 +20,2 @@", " ctx", "-removed", "+added"]));
    match &rows[1] {
        DiffRow::Split { left, right } => {
            assert_eq!(left.line_no, Some(10));
            assert_eq!(right.line_no, Some(20));
        }
        _ => panic!("expected split row"),
    }
    match &rows[2] {
        DiffRow::Split { left, right } => {
            assert_eq!(left.line_no, Some(11));
            assert_eq!(right.line_no, Some(21));
        }
        _ => panic!("expected split row"),
    }
}

#[test]
fn preamble_before_first_hunk_is_ignored() {
    let rows = parse_side_by_side(&lines(&[
        "diff --git a/f b/f",
        "index 111..222 100644",
        "--- a/f",
        "+++ b/f",
        "@@ -1,1 +1,1 @@",
        " ctx",
    ]));
    assert_eq!(rows.len(), 2);
    assert!(matches!(rows[0], DiffRow::Header(_)));
}

#[test]
fn no_newline_marker_is_skipped() {
    let rows = parse_side_by_side(&lines(&[
        "@@ -1,1 +1,1 @@",
        "-a",
        "\\ No newline at end of file",
        "+b",
    ]));
    assert_eq!(rows.len(), 2);
    match &rows[1] {
        DiffRow::Split { left, right } => {
            assert_eq!(left.text, "a");
            assert_eq!(right.text, "b");
        }
        _ => panic!("expected split row"),
    }
}

#[test]
fn removed_content_starting_with_dashes_is_not_a_header() {
    let rows = parse_side_by_side(&lines(&["@@ -1,1 +1,1 @@", "--- bullet", "+- bullet"]));
    match &rows[1] {
        DiffRow::Split { left, right } => {
            assert_eq!(left.text, "-- bullet");
            assert_eq!(right.text, "- bullet");
        }
        _ => panic!("expected split row"),
    }
}

#[test]
fn max_line_numbers_reports_widest_gutters() {
    let rows = parse_side_by_side(&lines(&["@@ -8,1 +120,1 @@", " ctx"]));
    assert_eq!(max_line_numbers(&rows), (8, 120));
}

#[test]
fn tokenize_splits_words_and_separators() {
    assert_eq!(tokenize("a_b = 1"), vec!["a_b", " = ", "1"]);
}

#[test]
fn word_diff_emphasizes_only_changed_tokens() {
    let (old_e, new_e) = word_diff("foo bar baz", "foo qux baz");
    let old_seg: String = old_e
        .iter()
        .flat_map(|&(s, e)| "foo bar baz".chars().skip(s).take(e - s))
        .collect();
    let new_seg: String = new_e
        .iter()
        .flat_map(|&(s, e)| "foo qux baz".chars().skip(s).take(e - s))
        .collect();
    assert_eq!(old_seg, "bar");
    assert_eq!(new_seg, "qux");
}

#[test]
fn word_diff_identical_lines_have_no_emphasis() {
    let (old_e, new_e) = word_diff("same line", "same line");
    assert!(old_e.is_empty());
    assert!(new_e.is_empty());
}

#[test]
fn push_range_merges_contiguous() {
    let mut r = Vec::new();
    push_range(&mut r, 0, 3);
    push_range(&mut r, 3, 5);
    push_range(&mut r, 7, 9);
    assert_eq!(r, vec![(0, 5), (7, 9)]);
}
