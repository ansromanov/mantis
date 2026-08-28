use super::*;

#[test]
fn maps_nested_object_and_array_lines() {
    let value = serde_json::json!({"spec": {"containers": [{"image": "app"}]}});
    let map = build_path_map(&value);
    assert!(map
        .iter()
        .flatten()
        .any(|path| path == ".spec.containers[0].image"));
}
