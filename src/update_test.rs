use super::*;

#[test]
fn test_parse_semver() {
    assert_eq!(parse_semver("0.15.3"), Some(vec![0, 15, 3]));
    assert_eq!(parse_semver("v0.15.3"), Some(vec![0, 15, 3]));
    assert_eq!(parse_semver("0.15.3-beta"), Some(vec![0, 15, 3]));
    assert_eq!(parse_semver("v1.0.0-rc1"), Some(vec![1, 0, 0]));
    assert_eq!(parse_semver("12.3.4.5"), Some(vec![12, 3, 4, 5]));
    assert_eq!(parse_semver("invalid"), None);
    assert_eq!(parse_semver(""), None);
    assert_eq!(parse_semver("v.1.0"), None);
}

#[test]
fn test_is_newer() {
    assert!(is_newer("0.16.0", "0.15.3"));
    assert!(is_newer("v0.16.0", "0.15.3"));
    assert!(is_newer("0.15.4", "0.15.3"));
    assert!(is_newer("1.0.0", "0.15.3"));
    assert!(is_newer("0.16.0-rc1", "0.15.3"));
    assert!(!is_newer("0.15.3", "0.15.3"));
    assert!(!is_newer("0.15.2", "0.15.3"));
    assert!(!is_newer("invalid", "0.15.3"));
    assert!(!is_newer("0.16.0", "invalid"));
}

#[test]
fn test_read_write_cache() {
    let temp_dir = tempfile::tempdir().unwrap();
    let cache_path = temp_dir.path().join(".update_cache");

    let cache = UpdateCache {
        last_checked: 123456789,
        latest_version: "v0.16.0".to_string(),
    };

    write_cache(&cache_path, &cache);
    let read = read_cache(&cache_path).unwrap();

    assert_eq!(read.last_checked, cache.last_checked);
    assert_eq!(read.latest_version, cache.latest_version);
}
