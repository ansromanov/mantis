use super::parse_blame_porcelain;

#[test]
fn single_line_single_commit() {
    let input = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1 1 1\n\
         author Alice\n\
         author-mail <alice@example.com>\n\
         author-time 1000000\n\
         author-tz +0000\n\
         committer Alice\n\
         committer-mail <alice@example.com>\n\
         committer-time 1000000\n\
         committer-tz +0000\n\
         summary init\n\
         filename src/foo.rs\n\
         \tfn main() {}\n";
    let result = parse_blame_porcelain(input);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].line_no, 1);
    assert_eq!(result[0].author, "Alice");
    assert_eq!(result[0].short_hash, "aaaaaaa");
    assert_eq!(
        result[0].commit_hash,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
}

#[test]
fn multi_line_same_commit() {
    let input = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1 1 3\n\
         author Alice\n\
         author-time 1000000\n\
         filename src/foo.rs\n\
         \tline one\n\
         aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 2 2\n\
         \tline two\n\
         aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 3 3\n\
         \tline three\n";
    let result = parse_blame_porcelain(input);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].line_no, 1);
    assert_eq!(result[1].line_no, 2);
    assert_eq!(result[2].line_no, 3);
    for b in &result {
        assert_eq!(b.author, "Alice");
    }
}

#[test]
fn multiple_commits_interleaved() {
    let input = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1 1 1\n\
         author Alice\n\
         author-time 1000000\n\
         filename src/foo.rs\n\
         \tline one\n\
         bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb 2 2 1\n\
         author Bob\n\
         author-time 2000000\n\
         filename src/foo.rs\n\
         \tline two\n\
         aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 3 3\n\
         \tline three\n";
    let result = parse_blame_porcelain(input);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].author, "Alice");
    assert_eq!(result[0].line_no, 1);
    assert_eq!(result[1].author, "Bob");
    assert_eq!(result[1].line_no, 2);
    assert_eq!(result[2].author, "Alice");
    assert_eq!(result[2].line_no, 3);
}

#[test]
fn empty_input() {
    assert!(parse_blame_porcelain("").is_empty());
}
