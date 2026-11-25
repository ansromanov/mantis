//! Saving a bug report to disk and building its GitHub issue title.

use super::App;

impl App {
    /// Collects an anonymous diagnostic report, saves it under the state directory,
    /// and opens the user's browser pre-filled with a GitHub new-issue URL.
    /// If the report exceeds ~6KB, truncates the URL query and copies the full
    /// report to the clipboard instead. If browser launch fails, falls back
    /// to clipboard copying and status bar instructions.
    pub(crate) fn save_bug_report(&mut self) {
        let title_text =
            resolve_bug_report_title(self.bug_report.as_ref().map(|br| br.title.as_str()));
        let report = crate::diagnostics::DiagnosticReport::collect(self);
        let md = report.to_markdown();
        let body_text = report.body.clone();

        // 1. Save locally first.
        let local_path = match report.save() {
            Ok(path) => Some(path),
            Err(e) => {
                self.set_status(format!("bug report local save failed: {e}"));
                None
            }
        };

        // 2. Prepare URL parameters.
        let base = "https://github.com/ansromanov/mantis/issues/new?template=app-bugreport.yml";
        let title_encoded = percent_encode(&title_text);

        let mut truncated = false;
        let (description_encoded, diagnostics_encoded) = {
            let desc_enc = percent_encode(&body_text);
            let diag_enc = percent_encode(&md);
            let test_url = format!(
                "{}&title={}&description={}&diagnostics={}",
                base, title_encoded, desc_enc, diag_enc
            );
            if test_url.len() > 6000 {
                truncated = true;
                let fallback_msg = format!(
                    "<!-- DIAGNOSTICS TOO LARGE FOR URL - COPIED TO CLIPBOARD. PLEASE PASTE OVER THIS LINE -->\n\
                     [Report truncated due to URL length limit ({} bytes). The full report was copied to your clipboard. Please paste it here.]",
                    md.len()
                );
                (percent_encode(&fallback_msg), percent_encode(""))
            } else {
                (desc_enc, diag_enc)
            }
        };

        let url = format!(
            "{}&title={}&description={}&diagnostics={}",
            base, title_encoded, description_encoded, diagnostics_encoded
        );

        // 3. Try to open browser (unless in a test context).
        let browser_res = if cfg!(test) {
            Ok(())
        } else {
            self.open_in_browser(&url)
        };

        // 4. Update status and clipboard depending on truncation and browser success.
        match (browser_res, truncated) {
            (Ok(()), false) => {
                if let Some(path) = local_path {
                    self.set_status(format!("bug report saved: {}", path.display()));
                } else {
                    self.set_status("opened browser");
                }
            }
            (Ok(()), true) => {
                self.copy_to_clipboard(md, "full bug report");
                self.set_status("opened browser; full report copied (paste into description)");
            }
            (Err(err), _) => {
                self.copy_to_clipboard(md, "full bug report");
                self.set_status(format!(
                    "clipboard filled; browser failed (open github.com/ansromanov/mantis/issues/new): {err}"
                ));
            }
        }

        self.bug_report = None;
    }
}

/// Resolves the GitHub issue title for a bug report: the trimmed user-entered
/// title, or the default when it is missing/blank.
pub(super) fn resolve_bug_report_title(title: Option<&str>) -> String {
    title
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .unwrap_or("App Bug Report")
        .to_string()
}

pub(super) fn percent_encode(s: &str) -> String {
    let mut encoded = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}
