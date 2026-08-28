//! Small, dependency-free JSON query language used by the JSON viewer.
//!
//! The query language intentionally covers only the operations useful in a
//! terminal log viewer: object/array navigation, array iteration, object
//! projection, and equality filtering. Parsing is kept separate from UI state
//! so malformed input can be rejected without replacing the last good result.
//! [`parse`] returns a validated [`Query`], while [`evaluate`] applies it to a
//! JSON value and returns zero or more projected values.

use serde_json::{Map, Value};

/// A parsed query in the deliberately small JSON-viewer grammar.
#[derive(Debug, Clone, PartialEq)]
pub enum Query {
    /// Navigate through object keys and array indices, optionally iterating an array.
    Path(Vec<Segment>),
    /// Project selected fields from the current object.
    Projection(Vec<String>),
    /// Keep the input when a path equals a JSON string, number, boolean, or null.
    Select { path: Vec<Segment>, expected: Value },
}

/// One step in a path expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Segment {
    /// Read an object member.
    Key(String),
    /// Read an array element by index.
    Index(usize),
    /// Yield each element of an array.
    Iterate,
}

/// Parses the supported query subset.
pub fn parse(input: &str) -> Result<Query, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("query is empty".into());
    }
    if let Some(inner) = input
        .strip_prefix("select(")
        .and_then(|s| s.strip_suffix(')'))
    {
        return parse_select(inner);
    }
    if input.starts_with('{') {
        return parse_projection(input);
    }
    Ok(Query::Path(parse_path(input)?))
}

fn parse_select(input: &str) -> Result<Query, String> {
    let Some((left, right)) = input.split_once("==") else {
        return Err("select requires ==".into());
    };
    let path = parse_path(left.trim())?;
    let expected = parse_json_literal(right.trim())?;
    Ok(Query::Select { path, expected })
}

fn parse_projection(input: &str) -> Result<Query, String> {
    if !input.ends_with('}') {
        return Err("projection is missing }".into());
    }
    let fields = input[1..input.len() - 1]
        .split(',')
        .map(str::trim)
        .map(|field| {
            if field.is_empty() || !field.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                Err(format!("invalid projection field: {field}"))
            } else {
                Ok(field.to_string())
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    if fields.is_empty() {
        return Err("projection has no fields".into());
    }
    Ok(Query::Projection(fields))
}

fn parse_path(input: &str) -> Result<Vec<Segment>, String> {
    let mut chars = input.chars().peekable();
    if chars.next() != Some('.') {
        return Err("path must start with .".into());
    }
    let mut segments = Vec::new();
    while chars.peek().is_some() {
        if chars.peek() == Some(&'.') {
            chars.next();
            if chars.peek().is_none() {
                return Err("path segment is empty".into());
            }
        }
        if chars.peek() == Some(&'[') {
            chars.next();
            if chars.peek() == Some(&']') {
                chars.next();
                segments.push(Segment::Iterate);
                continue;
            }
            let mut index = String::new();
            while let Some(&c) = chars.peek() {
                if c == ']' {
                    break;
                }
                if !c.is_ascii_digit() {
                    return Err("array index must be a non-negative integer".into());
                }
                index.push(c);
                chars.next();
            }
            if chars.next() != Some(']') || index.is_empty() {
                return Err("array index is missing ]".into());
            }
            segments.push(Segment::Index(
                index
                    .parse()
                    .map_err(|_| "array index is too large".to_string())?,
            ));
            continue;
        }
        let mut key = String::new();
        while let Some(&c) = chars.peek() {
            if c == '.' || c == '[' {
                break;
            }
            if !c.is_ascii_alphanumeric() && c != '_' && c != '-' {
                return Err(format!("invalid path character: {c}"));
            }
            key.push(c);
            chars.next();
        }
        if key.is_empty() {
            return Err("path segment is empty".into());
        }
        segments.push(Segment::Key(key));
    }
    if segments.is_empty() {
        return Err("path has no segments".into());
    }
    Ok(segments)
}

fn parse_json_literal(input: &str) -> Result<Value, String> {
    serde_json::from_str(input).map_err(|e| format!("invalid JSON literal: {e}"))
}

/// Evaluates a parsed query against one JSON document.
pub fn evaluate(query: &Query, value: &Value) -> Vec<Value> {
    match query {
        Query::Path(path) => walk(path, value),
        Query::Projection(fields) => {
            let Value::Object(object) = value else {
                return Vec::new();
            };
            vec![project(object, fields)]
        }
        Query::Select { path, expected } => {
            if walk(path, value)
                .iter()
                .any(|candidate| candidate == expected)
            {
                vec![value.clone()]
            } else {
                Vec::new()
            }
        }
    }
}

fn walk(path: &[Segment], value: &Value) -> Vec<Value> {
    if path.is_empty() {
        return vec![value.clone()];
    }
    match &path[0] {
        Segment::Key(key) => value
            .get(key)
            .map_or_else(Vec::new, |next| walk(&path[1..], next)),
        Segment::Index(index) => value
            .as_array()
            .and_then(|array| array.get(*index))
            .map_or_else(Vec::new, |next| walk(&path[1..], next)),
        Segment::Iterate => value
            .as_array()
            .map(|array| {
                array
                    .iter()
                    .flat_map(|item| walk(&path[1..], item))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn project(object: &Map<String, Value>, fields: &[String]) -> Value {
    let mut result = Map::new();
    for field in fields {
        if let Some(value) = object.get(field) {
            result.insert(field.clone(), value.clone());
        }
    }
    Value::Object(result)
}

#[cfg(test)]
#[path = "json_query_test.rs"]
mod tests;
