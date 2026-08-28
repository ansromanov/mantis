use super::*;
use std::path::Path;

#[test]
fn detects_csv_and_tsv_extensions() {
    assert!(is_csv_file(Path::new("data.csv")));
    assert!(is_csv_file(Path::new("data.tsv")));
    assert!(is_csv_file(Path::new("DATA.CSV")));
    assert!(is_csv_file(Path::new("DATA.TSV")));
    assert!(!is_csv_file(Path::new("data.txt")));
    assert!(!is_csv_file(Path::new("data.json")));
    assert!(!is_csv_file(Path::new("data")));
}

#[test]
fn detects_delimiter_for_path() {
    assert_eq!(delimiter_for_path(Path::new("data.csv")), ',');
    assert_eq!(delimiter_for_path(Path::new("data.tsv")), '\t');
    assert_eq!(delimiter_for_path(Path::new("DATA.TSV")), '\t');
    assert_eq!(delimiter_for_path(Path::new("data.txt")), ',');
}

#[test]
fn parses_empty_input() {
    assert!(parse_delimited("", ',').is_empty());
}

#[test]
fn parses_simple_csv() {
    let raw = "name,age,city\nAlice,30,New York\nBob,25,London\n";
    let rows = parse_delimited(raw, ',');
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0], vec!["name", "age", "city"]);
    assert_eq!(rows[1], vec!["Alice", "30", "New York"]);
    assert_eq!(rows[2], vec!["Bob", "25", "London"]);
}

#[test]
fn parses_simple_tsv() {
    let raw = "id\tname\tscore\n1\tAlice\t95\n2\tBob\t88";
    let rows = parse_delimited(raw, '\t');
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0], vec!["id", "name", "score"]);
    assert_eq!(rows[1], vec!["1", "Alice", "95"]);
    assert_eq!(rows[2], vec!["2", "Bob", "88"]);
}

#[test]
fn handles_quoted_fields_with_delimiters() {
    let raw = r#"name,description,count
"Apples, Red",Fresh fruit,10
"Oranges, Sweet",Citrus,20"#;
    let rows = parse_delimited(raw, ',');
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[1], vec!["Apples, Red", "Fresh fruit", "10"]);
    assert_eq!(rows[2], vec!["Oranges, Sweet", "Citrus", "20"]);
}

#[test]
fn handles_escaped_double_quotes() {
    let raw = r#"id,quote
1,"He said ""hello"" to everyone"
2,"Single "" quote""#;
    let rows = parse_delimited(raw, ',');
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[1][1], r#"He said "hello" to everyone"#);
    assert_eq!(rows[2][1], r#"Single " quote"#);
}

#[test]
fn handles_embedded_newlines_in_quotes() {
    let raw = "col1,col2\n\"line1\nline2\",val2\n\"a\r\nb\",val3";
    let rows = parse_delimited(raw, ',');
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[1][0], "line1↵line2");
    assert_eq!(rows[2][0], "a↵b");
}

#[test]
fn pads_ragged_rows() {
    let raw = "a,b,c,d\n1,2\n3,4,5";
    let rows = parse_delimited(raw, ',');
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0], vec!["a", "b", "c", "d"]);
    assert_eq!(rows[1], vec!["1", "2", "", ""]);
    assert_eq!(rows[2], vec!["3", "4", "5", ""]);
}

#[test]
fn handles_crlf_and_cr_line_endings() {
    let raw = "a,b\r\n1,2\r3,4";
    let rows = parse_delimited(raw, ',');
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0], vec!["a", "b"]);
    assert_eq!(rows[1], vec!["1", "2"]);
    assert_eq!(rows[2], vec!["3", "4"]);
}

#[test]
fn formats_empty_table() {
    let lines = format_table_lines(&[]);
    assert_eq!(lines, vec!["[empty table]"]);
}

#[test]
fn formats_single_row_table() {
    let rows = vec![vec!["Name".into(), "Age".into()]];
    let lines = format_table_lines(&rows);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "┌──────┬─────┐");
    assert_eq!(lines[1], "│ Name │ Age │");
    assert_eq!(lines[2], "└──────┴─────┘");
}

#[test]
fn formats_multi_row_table_with_alignment() {
    let rows = vec![
        vec!["Name".into(), "Age".into(), "City".into()],
        vec!["Alice".into(), "30".into(), "New York".into()],
        vec!["Bob".into(), "7".into(), "Paris".into()],
    ];
    let lines = format_table_lines(&rows);
    assert_eq!(lines.len(), 6);
    assert_eq!(lines[0], "┌───────┬─────┬──────────┐");
    assert_eq!(lines[1], "│ Name  │ Age │ City     │");
    assert_eq!(lines[2], "├───────┼─────┼──────────┤");
    assert_eq!(lines[3], "│ Alice │ 30  │ New York │");
    assert_eq!(lines[4], "│ Bob   │ 7   │ Paris    │");
    assert_eq!(lines[5], "└───────┴─────┴──────────┘");
}

#[test]
fn formats_table_with_unicode_characters() {
    let rows = vec![
        vec!["Item".into(), "Icon".into()],
        vec!["Apple".into(), "🍎".into()],
        vec!["Car".into(), "🚗".into()],
    ];
    let lines = format_table_lines(&rows);
    assert_eq!(lines.len(), 6);
    assert_eq!(lines[0], "┌───────┬──────┐");
    assert_eq!(lines[1], "│ Item  │ Icon │");
    assert_eq!(lines[2], "├───────┼──────┤");
    assert_eq!(lines[3], "│ Apple │ 🍎   │");
    assert_eq!(lines[4], "│ Car   │ 🚗   │");
    assert_eq!(lines[5], "└───────┴──────┘");
}
