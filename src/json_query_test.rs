use super::*;

use serde_json::json;

#[test]
fn parses_paths_and_array_iteration() {
    assert_eq!(
        parse(".items[].name").unwrap(),
        Query::Path(vec![
            Segment::Key("items".into()),
            Segment::Iterate,
            Segment::Key("name".into()),
        ])
    );
    assert_eq!(
        parse(".items[0].name").unwrap(),
        Query::Path(vec![
            Segment::Key("items".into()),
            Segment::Index(0),
            Segment::Key("name".into()),
        ])
    );
}

#[test]
fn evaluates_nested_path_and_projection() {
    let value = json!({"items": [{"name": "one", "n": 1}, {"name": "two", "n": 2}]});
    let names = evaluate(&parse(".items[].name").unwrap(), &value);
    assert_eq!(names, vec![json!("one"), json!("two")]);

    let projected = evaluate(&parse("{name, n}").unwrap(), &value["items"][0]);
    assert_eq!(projected, vec![json!({"name": "one", "n": 1})]);
}

#[test]
fn evaluates_select_for_jsonl_style_records() {
    let query = parse("select(.level == \"error\")").unwrap();
    assert_eq!(
        evaluate(&query, &json!({"level": "error", "msg": "bad"})),
        vec![json!({"level": "error", "msg": "bad"})]
    );
    assert!(evaluate(&query, &json!({"level": "info"})).is_empty());
}

#[test]
fn rejects_unsupported_or_malformed_queries() {
    for query in [
        "",
        "items",
        ".items[",
        ".items[nope]",
        "select(.level)",
        "{}",
    ] {
        assert!(parse(query).is_err(), "query should fail: {query}");
    }
}
