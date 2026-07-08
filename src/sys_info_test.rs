use super::*;

#[test]
fn test_whitelisted_env() {
    let test_key = "MANTIS_TEST_ENV_VAR_XYZ";
    assert!(whitelisted_env(test_key).is_none());

    std::env::set_var(test_key, "hello");
    assert_eq!(whitelisted_env(test_key), Some("hello".to_string()));

    std::env::set_var(test_key, "");
    assert!(whitelisted_env(test_key).is_none());

    std::env::remove_var(test_key);
}

#[test]
fn test_os_version_does_not_panic() {
    let version = os_version();
    if cfg!(any(target_os = "linux", target_os = "macos")) {
        // We might not get a version in every test environment,
        // but we can call it to ensure it does not panic.
        let _ = version;
    }
}

#[test]
fn test_is_wsl_does_not_panic() {
    let _ = is_wsl();
}
