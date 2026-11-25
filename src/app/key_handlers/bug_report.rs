//! Key handling for the bug-report form.

use crossterm::event::{KeyCode, KeyEvent};

use super::super::App;

impl App {
    pub(super) fn handle_bug_report_key(&mut self, key: KeyEvent) {
        use crossterm::event::KeyModifiers;

        let is_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let is_alt = key.modifiers.contains(KeyModifiers::ALT);

        // Submit/Save: Ctrl+S or Ctrl+Enter
        if is_ctrl
            && (key.code == KeyCode::Char('s')
                || key.code == KeyCode::Char('S')
                || key.code == KeyCode::Enter)
        {
            self.save_bug_report();
            return;
        }

        use crate::search::BugReportFocus;

        match key.code {
            KeyCode::Esc => {
                self.bug_report = None;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                if let Some(ref mut state) = self.bug_report {
                    state.toggle_focus();
                }
            }
            KeyCode::Enter => {
                if let Some(ref mut state) = self.bug_report {
                    match state.focus {
                        BugReportFocus::Title => state.focus = BugReportFocus::Description,
                        BugReportFocus::Description => state.insert_newline(),
                    }
                }
            }
            KeyCode::Backspace => {
                if let Some(ref mut state) = self.bug_report {
                    match state.focus {
                        BugReportFocus::Title => state.title_backspace(),
                        BugReportFocus::Description => state.backspace(),
                    }
                }
            }
            KeyCode::Delete => {
                if let Some(ref mut state) = self.bug_report {
                    match state.focus {
                        BugReportFocus::Title => state.title_delete(),
                        BugReportFocus::Description => state.delete(),
                    }
                }
            }
            KeyCode::Left => {
                if let Some(ref mut state) = self.bug_report {
                    match state.focus {
                        BugReportFocus::Title => state.title_move_left(),
                        BugReportFocus::Description => state.move_left(),
                    }
                }
            }
            KeyCode::Right => {
                if let Some(ref mut state) = self.bug_report {
                    match state.focus {
                        BugReportFocus::Title => state.title_move_right(),
                        BugReportFocus::Description => state.move_right(),
                    }
                }
            }
            KeyCode::Up => {
                if let Some(ref mut state) = self.bug_report {
                    match state.focus {
                        BugReportFocus::Title => {}
                        BugReportFocus::Description if state.cursor_row == 0 => {
                            state.focus = BugReportFocus::Title;
                        }
                        BugReportFocus::Description => state.move_up(),
                    }
                }
            }
            KeyCode::Down => {
                if let Some(ref mut state) = self.bug_report {
                    match state.focus {
                        BugReportFocus::Title => state.focus = BugReportFocus::Description,
                        BugReportFocus::Description => state.move_down(),
                    }
                }
            }
            KeyCode::Home => {
                if let Some(ref mut state) = self.bug_report {
                    match state.focus {
                        BugReportFocus::Title => state.title_move_home(),
                        BugReportFocus::Description => state.move_home(),
                    }
                }
            }
            KeyCode::End => {
                if let Some(ref mut state) = self.bug_report {
                    match state.focus {
                        BugReportFocus::Title => state.title_move_end(),
                        BugReportFocus::Description => state.move_end(),
                    }
                }
            }
            KeyCode::PageUp => {
                if let Some(ref mut state) = self.bug_report {
                    state.preview_scroll.scroll_up(10);
                }
            }
            KeyCode::PageDown => {
                if let Some(ref mut state) = self.bug_report {
                    let body_text = state.text.join("\n");
                    let report_md = if body_text.trim().is_empty() {
                        state.diagnostics_markdown.clone()
                    } else {
                        format!(
                            "## {}\n\n## bug report body\n\n{}\n\n{}",
                            state.resolved_title(),
                            body_text,
                            state.diagnostics_markdown
                        )
                    };
                    let lines_count = report_md.lines().count();
                    let height = self.geo.bug_report_preview_area.height.max(1) as usize;
                    let max_scroll = lines_count.saturating_sub(height);
                    state.preview_scroll.scroll_down(10, max_scroll);
                }
            }
            KeyCode::Char(c) if !is_ctrl && !is_alt => {
                if let Some(ref mut state) = self.bug_report {
                    match state.focus {
                        BugReportFocus::Title => state.title_insert_char(c),
                        BugReportFocus::Description => state.insert_char(c),
                    }
                }
            }
            _ => {}
        }
    }
}
