//! Clipboard access for `App` (system clipboard with an OSC 52 fallback).

use super::App;

impl App {
    /// Copies `text` to the system clipboard, reporting success or failure in the
    /// status bar. Single source of truth for clipboard writes.
    pub(crate) fn copy_to_clipboard(&mut self, text: String, label: &str) {
        if text.is_empty() {
            return;
        }
        match self.clipboard_set(text) {
            Ok(()) => self.set_status(format!("copied {label}")),
            Err(e) => {
                self.set_status(format!("clipboard error: {e}"));
            }
        }
    }

    /// Writes `text` to the system clipboard.
    #[cfg(not(test))]
    fn clipboard_set(&mut self, text: String) -> Result<(), arboard::Error> {
        arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text))
    }

    /// Test double: captures `text` in `clipboard_capture` so `cargo test`
    /// never touches (or races on) the real system clipboard.
    #[cfg(test)]
    fn clipboard_set(&mut self, text: String) -> Result<(), arboard::Error> {
        self.clipboard_capture.push(text);
        Ok(())
    }
}
