//! Anonymous diagnostic bug reports, collected on demand and saved locally.
//!
//! `DiagnosticReport::collect` snapshots the facts a maintainer needs to
//! reproduce a problem — app version, OS and terminal identity, the *shape*
//! of the workspace (node counts, depth), facts about the open file (size,
//! extension, encoding), and which config keys differ from defaults — while
//! deliberately excluding anything personal: no absolute paths, no file or
//! plugin names, no file content, no config values. The report is a closed
//! struct of typed fields, so nothing can leak in by accident; the sibling
//! test file asserts the serialized form never contains the workspace root,
//! the open file's name, or the home directory. `save` writes the rendered
//! markdown to `<state_dir>/bug-reports/` so the user can review and attach
//! it to a GitHub issue. The in-app "Report a bug" palette command
//! (`app::key_handlers::editor::dispatch_command`) is the single caller.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::app::App;
use crate::config::Config;

/// Everything included in a bug report. Counts, whitelisted identifiers, and
/// booleans only — see the module doc for the privacy rules.
#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticReport {
    // Application.
    pub app_version: &'static str,
    pub release_date: Option<String>,
    // Operating system.
    pub os: &'static str,
    pub arch: &'static str,
    pub os_version: Option<String>,
    pub wsl: bool,
    // Terminal (whitelisted env vars; never a dump of the environment).
    pub term: Option<String>,
    pub term_program: Option<String>,
    pub term_program_version: Option<String>,
    pub colorterm: Option<String>,
    pub windows_terminal: bool,
    pub ssh_session: bool,
    pub terminal_size: Option<(u16, u16)>,
    // Workspace shape (counts only, no names or paths).
    pub tree_nodes: usize,
    pub tree_files: usize,
    pub tree_dirs: usize,
    pub tree_max_depth: usize,
    pub expanded_dirs: usize,
    pub tree_filter_active: bool,
    pub walk_errors: usize,
    pub git_repo: bool,
    pub git_mode: bool,
    // Open file facts (extension only, never the name).
    pub file_open: bool,
    pub file_extension: Option<String>,
    pub file_size_bytes: Option<u64>,
    pub file_line_count: Option<usize>,
    pub file_encoding: Option<String>,
    pub file_line_ending: Option<String>,
    pub file_syntax: Option<String>,
    pub file_is_json: bool,
    pub file_is_diff: bool,
    pub file_uses_mmap: bool,
    // Configuration (key paths that differ from defaults; values omitted).
    pub theme: Option<String>,
    pub config_overrides: Vec<String>,
    pub plugin_count: usize,
    pub telemetry_enabled: bool,
}

impl DiagnosticReport {
    /// Snapshots the current app state and environment. Read-only.
    pub fn collect(app: &App) -> Self {
        let file_size_bytes = app
            .current_file
            .as_deref()
            .and_then(|p| fs::metadata(p).ok())
            .map(|m| m.len());
        DiagnosticReport {
            app_version: env!("CARGO_PKG_VERSION"),
            release_date: crate::release_info::RELEASE
                .as_ref()
                .map(|r| r.date.clone()),
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            os_version: os_version(),
            wsl: is_wsl(),
            term: whitelisted_env("TERM"),
            term_program: whitelisted_env("TERM_PROGRAM"),
            term_program_version: whitelisted_env("TERM_PROGRAM_VERSION"),
            colorterm: whitelisted_env("COLORTERM"),
            windows_terminal: std::env::var_os("WT_SESSION").is_some(),
            ssh_session: std::env::var_os("SSH_CONNECTION").is_some(),
            terminal_size: crossterm::terminal::size().ok(),
            tree_nodes: app.nodes.len(),
            tree_files: app.nodes.iter().filter(|n| !n.is_dir).count(),
            tree_dirs: app.nodes.iter().filter(|n| n.is_dir).count(),
            tree_max_depth: app.nodes.iter().map(|n| n.depth).max().unwrap_or(0),
            expanded_dirs: app.expanded.len(),
            tree_filter_active: app.tree_filter.is_some(),
            walk_errors: app.walk_errors,
            git_repo: app.git_info.is_some(),
            git_mode: app.git_mode,
            file_open: app.current_file.is_some(),
            file_extension: app
                .current_file
                .as_deref()
                .and_then(|p| p.extension())
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase()),
            file_size_bytes,
            file_line_count: app.current_file.is_some().then(|| app.line_count()),
            file_encoding: app.file_encoding.clone(),
            file_line_ending: app.file_line_ending.clone(),
            file_syntax: app.current_syntax.clone(),
            file_is_json: app.is_json,
            file_is_diff: app.is_diff,
            file_uses_mmap: app.virtual_file.is_some(),
            theme: app.config.theme.name.clone(),
            config_overrides: changed_config_paths(&app.config),
            plugin_count: app.config.plugins.len(),
            telemetry_enabled: app.telemetry.is_enabled(),
        }
    }

    /// Renders the report as the markdown body a user can paste into an issue.
    pub fn to_markdown(&self) -> String {
        let mut md = String::from("## mantis diagnostic report\n\n");
        let opt = |v: &Option<String>| v.clone().unwrap_or_else(|| "unknown".into());
        md.push_str(&format!(
            "- **app**: {} (released {})\n",
            self.app_version,
            opt(&self.release_date)
        ));
        md.push_str(&format!(
            "- **os**: {} {} — {}{}\n",
            self.os,
            self.arch,
            opt(&self.os_version),
            if self.wsl { " (WSL)" } else { "" }
        ));
        let size = self
            .terminal_size
            .map(|(w, h)| format!("{w}x{h}"))
            .unwrap_or_else(|| "unknown".into());
        md.push_str(&format!(
            "- **terminal**: TERM={} program={} {} colorterm={} size={}{}{}\n",
            opt(&self.term),
            opt(&self.term_program),
            opt(&self.term_program_version),
            opt(&self.colorterm),
            size,
            if self.windows_terminal {
                " windows-terminal"
            } else {
                ""
            },
            if self.ssh_session { " ssh" } else { "" }
        ));
        md.push_str(&format!(
            "- **workspace**: {} nodes ({} files / {} dirs), max depth {}, {} expanded, \
             filter={}, walk errors={}, git repo={}, git mode={}\n",
            self.tree_nodes,
            self.tree_files,
            self.tree_dirs,
            self.tree_max_depth,
            self.expanded_dirs,
            self.tree_filter_active,
            self.walk_errors,
            self.git_repo,
            self.git_mode
        ));
        if self.file_open {
            md.push_str(&format!(
                "- **open file**: ext={} size={} lines={} encoding={} line-endings={} \
                 syntax={} json={} diff={} mmap={}\n",
                opt(&self.file_extension),
                self.file_size_bytes
                    .map(|b| b.to_string())
                    .unwrap_or_else(|| "unknown".into()),
                self.file_line_count
                    .map(|l| l.to_string())
                    .unwrap_or_else(|| "unknown".into()),
                opt(&self.file_encoding),
                opt(&self.file_line_ending),
                opt(&self.file_syntax),
                self.file_is_json,
                self.file_is_diff,
                self.file_uses_mmap
            ));
        } else {
            md.push_str("- **open file**: none\n");
        }
        md.push_str(&format!("- **theme**: {}\n", opt(&self.theme)));
        md.push_str(&format!(
            "- **config overrides**: {}\n",
            if self.config_overrides.is_empty() {
                "none".to_string()
            } else {
                self.config_overrides.join(", ")
            }
        ));
        md.push_str(&format!("- **plugins**: {}\n", self.plugin_count));
        md.push_str(&format!(
            "- **telemetry**: {}\n",
            if self.telemetry_enabled {
                "enabled"
            } else {
                "disabled"
            }
        ));
        md
    }

    /// Writes the rendered report to `<state_dir>/bug-reports/report-<epoch>.md`
    /// and returns the path. Errors bubble up so the caller can surface them.
    pub fn save(&self) -> std::io::Result<PathBuf> {
        let dir = crate::session::state_dir()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no state directory"))?
            .join("bug-reports");
        fs::create_dir_all(&dir)?;
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut path = dir.join(format!("report-{epoch}.md"));
        let mut n = 1;
        while path.exists() {
            path = dir.join(format!("report-{epoch}-{n}.md"));
            n += 1;
        }
        fs::write(&path, self.to_markdown())?;
        Ok(path)
    }
}

/// Reads a whitelisted terminal-identity env var. The whitelist lives in
/// `DiagnosticReport::collect`; this helper never sees arbitrary var names
/// from user input.
fn whitelisted_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// Best-effort human-readable OS version. `None` on platforms without a
/// cheap, reliable probe.
fn os_version() -> Option<String> {
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

/// Whether we are running under Windows Subsystem for Linux, which has its
/// own set of terminal quirks worth knowing about in a report.
fn is_wsl() -> bool {
    cfg!(target_os = "linux")
        && fs::read_to_string("/proc/version").is_ok_and(|v| v.to_lowercase().contains("microsoft"))
}

/// Dotted key paths where `cfg` differs from `Config::default()`. Only the
/// *paths* are reported, never the values, so string-typed settings cannot
/// leak. The `plugins` table is collapsed to a single `plugins` entry because
/// its keys are user-chosen plugin names.
fn changed_config_paths(cfg: &Config) -> Vec<String> {
    let (Ok(current), Ok(default)) = (
        toml::Value::try_from(cfg.clone()),
        toml::Value::try_from(Config::default()),
    ) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    diff_value("", &current, &default, &mut out);
    out.sort();
    out
}

fn diff_value(prefix: &str, current: &toml::Value, default: &toml::Value, out: &mut Vec<String>) {
    match (current, default) {
        (toml::Value::Table(cur), toml::Value::Table(def)) => {
            for (key, value) in cur {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                if path == "plugins" {
                    if !matches!(value, toml::Value::Table(t) if t.is_empty()) {
                        out.push("plugins".to_string());
                    }
                    continue;
                }
                match def.get(key) {
                    Some(default_value) => diff_value(&path, value, default_value, out),
                    None => out.push(path),
                }
            }
        }
        _ => {
            if current != default {
                out.push(prefix.to_string());
            }
        }
    }
}

#[cfg(test)]
#[path = "diagnostics_test.rs"]
mod tests;
