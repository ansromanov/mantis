//! Optional background update checking and self-updating.
//!
//! Checks GitHub Releases for a version newer than the compile-time metadata,
//! at most once per day. Caches results in the config directory to avoid API
//! rate-limits. If an update is available, displays a notice in the status
//! bar and About screen. Provides a `--update` subcommand to download and
//! install the latest version.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateCache {
    pub last_checked: u64,
    pub latest_version: String,
}

/// Resolves the update cache path, located next to the user's config file.
pub fn get_cache_path() -> Option<PathBuf> {
    crate::plugin::default_plugin_dir()
        .parent()
        .map(|p| p.join(".update_cache"))
}

/// Reads the update cache file.
pub fn read_cache(path: &Path) -> Option<UpdateCache> {
    let s = fs::read_to_string(path).ok()?;
    let mut lines = s.lines();
    let last_checked = lines.next()?.parse::<u64>().ok()?;
    let latest_version = lines.next()?.trim().to_string();
    Some(UpdateCache {
        last_checked,
        latest_version,
    })
}

/// Writes the update cache file.
pub fn write_cache(path: &Path, cache: &UpdateCache) {
    let content = format!("{}\n{}\n", cache.last_checked, cache.latest_version);
    let _ = fs::write(path, content);
}

/// Parses a semver version string into parts of numbers.
/// Tolerates leading 'v' prefix and trailing alphanumeric tags.
pub fn parse_semver(v: &str) -> Option<Vec<u64>> {
    let clean = v.strip_prefix('v').unwrap_or(v);
    let mut parts = Vec::new();
    for s in clean.split('.') {
        let end_idx = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
        let num_str = &s[..end_idx];
        if num_str.is_empty() {
            return None;
        }
        parts.push(num_str.parse::<u64>().ok()?);
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts)
    }
}

/// Returns true if the latest version is newer than the current version.
pub fn is_newer(latest: &str, current: &str) -> bool {
    let latest_parts = parse_semver(latest);
    let current_parts = parse_semver(current);
    match (latest_parts, current_parts) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

/// Fetches the latest version tag from the GitHub API using native tools (curl, wget, or powershell).
pub fn fetch_latest_version_tag() -> Option<String> {
    #[cfg(unix)]
    {
        use std::process::Command;
        // Try curl first
        if let Ok(output) = Command::new("curl")
            .args([
                "-s",
                "-H",
                "User-Agent: mantis",
                "https://api.github.com/repos/ansromanov/mantis/releases/latest",
            ])
            .output()
        {
            if output.status.success() {
                if let Ok(s) = String::from_utf8(output.stdout) {
                    if let Some(tag) = parse_tag_from_json(&s) {
                        return Some(tag);
                    }
                }
            }
        }
        // Try wget
        if let Ok(output) = Command::new("wget")
            .args([
                "-qO-",
                "--header=User-Agent: mantis",
                "https://api.github.com/repos/ansromanov/mantis/releases/latest",
            ])
            .output()
        {
            if output.status.success() {
                if let Ok(s) = String::from_utf8(output.stdout) {
                    if let Some(tag) = parse_tag_from_json(&s) {
                        return Some(tag);
                    }
                }
            }
        }
    }
    #[cfg(windows)]
    {
        use std::process::Command;
        if let Ok(output) = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; $r = Invoke-RestMethod -Uri 'https://api.github.com/repos/ansromanov/mantis/releases/latest' -Headers @{'User-Agent'='mantis'}; $r.tag_name"
            ])
            .output()
        {
            if output.status.success() {
                if let Ok(s) = String::from_utf8(output.stdout) {
                    let tag = s.trim().to_string();
                    if !tag.is_empty() {
                        return Some(tag);
                    }
                }
            }
        }
    }
    None
}

fn parse_tag_from_json(json_str: &str) -> Option<String> {
    let val: serde_json::Value = serde_json::from_str(json_str).ok()?;
    val.get("tag_name")?.as_str().map(|s| s.to_string())
}

/// Triggers the background check for updates if a day has passed since the last check.
/// Returns a tuple of (instant version notice if cached, background channel receiver for latest).
pub fn check_for_updates(
    check_enabled: bool,
) -> (Option<String>, Option<std::sync::mpsc::Receiver<String>>) {
    if cfg!(test) || !check_enabled {
        return (None, None);
    }
    let Some(cache_path) = get_cache_path() else {
        return (None, None);
    };

    let cache = read_cache(&cache_path);
    let current_version = crate::release_info::RELEASE
        .as_ref()
        .map(|r| r.version.as_str())
        .unwrap_or(env!("CARGO_PKG_VERSION"));

    let mut new_version_available = None;
    if let Some(ref c) = cache {
        if is_newer(&c.latest_version, current_version) {
            new_version_available = Some(c.latest_version.clone());
        }
    }

    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let last_checked = cache.as_ref().map(|c| c.last_checked).unwrap_or(0);

    let update_rx = if now_secs.saturating_sub(last_checked) >= 86400 {
        let (tx, rx) = std::sync::mpsc::channel();
        let current_version = current_version.to_string();
        std::thread::spawn(move || {
            if let Some(latest) = fetch_latest_version_tag() {
                write_cache(
                    &cache_path,
                    &UpdateCache {
                        last_checked: now_secs,
                        latest_version: latest.clone(),
                    },
                );
                if is_newer(&latest, &current_version) {
                    let _ = tx.send(latest);
                }
            } else if let Some(mut c) = cache {
                c.last_checked = now_secs;
                write_cache(&cache_path, &c);
            } else {
                write_cache(
                    &cache_path,
                    &UpdateCache {
                        last_checked: now_secs,
                        latest_version: String::new(),
                    },
                );
            }
        });
        Some(rx)
    } else {
        None
    };

    (new_version_available, update_rx)
}

/// Runs the checksum-verified self-update flow from GitHub.
pub fn run_self_update() -> anyhow::Result<()> {
    println!("Checking for updates and running the installer...");
    #[cfg(unix)]
    {
        use std::process::Command;
        // Pipe curl to sh, with wget fallback.
        let status = Command::new("sh")
            .arg("-c")
            .arg("curl -fsSL https://raw.githubusercontent.com/ansromanov/mantis/main/install.sh | sh || wget -qO- https://raw.githubusercontent.com/ansromanov/mantis/main/install.sh | sh")
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()?;
        if !status.success() {
            anyhow::bail!("Installer failed with exit status: {status}");
        }
    }
    #[cfg(windows)]
    {
        use std::process::Command;
        let status = Command::new("powershell")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "irm https://raw.githubusercontent.com/ansromanov/mantis/main/install.ps1 | iex",
            ])
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()?;
        if !status.success() {
            anyhow::bail!("Installer failed with exit status: {status}");
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "update_test.rs"]
mod tests;
