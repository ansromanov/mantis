use super::*;

use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;

#[test]
fn detects_credential_filenames_and_masks_fixed_width() {
    assert!(credential_filename(Path::new(".env.production")));
    assert_eq!(
        mask_line("API_TOKEN=long-secret-value"),
        "API_TOKEN=********"
    );
}

#[test]
fn detects_pem_and_cloud_tokens_but_not_normal_password_text() {
    assert!(secret_line(
        ["-----BEGIN RSA ", "PRIVATE KEY-----"].concat().as_str()
    ));
    assert!(secret_line("value=ghp_123456789"));
    assert!(!secret_line("README=passwords are documented here"));
    assert!(secret_line("token: ghp_123456789"));
    assert!(secret_line("ghp_123456789"));
}

#[test]
fn content_detection_masks_without_leaking_value_length() {
    let lines = vec!["name=ok".into(), "AWS_SECRET_ACCESS_KEY=abc".into()];
    let (masked, detected) = mask_lines(Path::new("notes.txt"), &lines, true);
    assert!(detected);
    assert_eq!(masked[1], "AWS_SECRET_ACCESS_KEY=********");
}

#[test]
fn content_probe_detects_secret_shape_in_an_ordinary_file() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "notes=ok").unwrap();
    writeln!(file, "SERVICE_TOKEN=secret-value").unwrap();
    assert!(content_probe(file.path()));
}

#[test]
fn masks_private_key_body_and_structured_secret_values() {
    let lines = vec![
        ["-----BEGIN ", "PRIVATE KEY-----"].concat(),
        "private-key-body".into(),
        ["-----END ", "PRIVATE KEY-----"].concat(),
        "password: visible-value".into(),
    ];
    let (masked, detected) = mask_lines(Path::new("key.pem"), &lines, true);
    assert!(detected);
    assert_eq!(
        masked,
        vec!["********", "********", "********", "password: ********"]
    );
}
