//! Shared system-information helpers used by both [`crate::diagnostics`] and
//! [`crate::telemetry`] to avoid duplicating OS-version, WSL-detection, and
//! whitelisted-env-var logic.
//!
//! Privacy rules: `whitelisted_env` only queries a fixed set of
//! terminal-identity variables (TERM, TERM_PROGRAM, …) that carry no personal
//! data; the whitelist is maintained externally by the callers. `os_version`
//! and `is_wsl` probe static platform properties only. Public items:
//! [`whitelisted_env`], [`os_version`], [`is_wsl`].

use std::fs;

/// Reads a whitelisted terminal-identity env var. The whitelist lives in the
/// caller's field list; this helper never sees arbitrary var names from user
/// input.
pub(crate) fn whitelisted_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// Best-effort human-readable OS version. `None` on platforms without a
/// cheap, reliable probe.
pub(crate) fn os_version() -> Option<String> {
    if cfg!(target_os = "linux") {
        let raw = fs::read_to_string("/etc/os-release").ok()?;
        raw.lines()
            .find_map(|l| l.strip_prefix("PRETTY_NAME="))
            .map(|v| v.trim_matches('"').to_string())
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| format!("macOS {}", s.trim()))
    } else {
        None
    }
}

/// Whether we are running under Windows Subsystem for Linux.
pub(crate) fn is_wsl() -> bool {
    cfg!(target_os = "linux")
        && fs::read_to_string("/proc/version").is_ok_and(|v| v.to_lowercase().contains("microsoft"))
}

#[cfg(test)]
#[path = "sys_info_test.rs"]
mod tests;
