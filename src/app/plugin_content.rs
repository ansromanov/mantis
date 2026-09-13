//! Plugin-rendered content installation and stream lifecycle.
//!
//! Process plugins can replace a file with one `set_content` action or deliver
//! large rendered output as indexed chunks. The chunk path validates IDs and
//! ranges, appends only contiguous batches, keeps the content query APIs in
//! sync, and marks stalled or over-limit output incomplete without discarding
//! lines already shown. The module owns the content actions on `App`, the
//! timeout check called from `tick`, and the state transitions shared by the
//! status bar and content pane.

use std::path::{Path, PathBuf};
use std::time::Instant;

use super::App;

impl App {
    pub(super) fn handle_plugin_set_content(&mut self, name: &str, params: &serde_json::Value) {
        let lines: Vec<String> = match params.get("lines").and_then(|v| v.as_array()) {
            Some(arr) => arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            None => return,
        };
        let path = match params.get("path").and_then(|v| v.as_str()) {
            Some(p) => PathBuf::from(p),
            None => return,
        };
        if self
            .plugin_content_streams
            .get(&path)
            .is_some_and(|(owner, _)| owner == name)
        {
            self.plugin_content_streams.remove(&path);
        }
        let rendered: Vec<Vec<(ratatui::style::Style, String)>> = lines
            .iter()
            .map(|line| crate::ansi::parse_ansi_line(line))
            .collect();
        let text: Vec<String> = rendered
            .iter()
            .map(|spans| spans.iter().map(|(_, value)| value.as_str()).collect())
            .collect();
        let is_current = self.current_file.as_deref() == Some(path.as_path());
        self.plugin_content_text.insert(path.clone(), text);
        self.plugin_content.insert(path.clone(), rendered);
        self.plugin_contributions
            .entry(name.to_string())
            .or_default()
            .content_paths
            .insert(path.clone());
        if is_current {
            self.activate_plugin_content(&path);
        }
    }

    pub(super) fn handle_plugin_content_chunk(&mut self, name: &str, params: &serde_json::Value) {
        let Some(path) = params
            .get("path")
            .and_then(|value| value.as_str())
            .map(PathBuf::from)
        else {
            return;
        };
        let Some(content_id) = params.get("content_id").and_then(|value| value.as_str()) else {
            return;
        };
        if content_id.is_empty() || content_id.len() > 128 {
            return;
        }
        let Some(index) = params
            .get("index")
            .and_then(serde_json::Value::as_u64)
            .and_then(|index| usize::try_from(index).ok())
        else {
            return;
        };
        let Some(lines) = params.get("lines").and_then(serde_json::Value::as_array) else {
            return;
        };
        let Some(lines) = lines
            .iter()
            .map(|line| line.as_str().map(str::to_string))
            .collect::<Option<Vec<_>>>()
        else {
            return;
        };

        self.ensure_plugin_content_stream(name, &path, content_id);
        let Some((owner, stream)) = self.plugin_content_streams.get_mut(&path) else {
            return;
        };
        if owner != name {
            return;
        }
        match stream.push_chunk(index, lines, Instant::now()) {
            Ok(update) => self.append_plugin_content_lines(name, &path, update.appended),
            Err(error) => {
                self.plugin_message =
                    Some(format!("[{name}] rendered content incomplete ({error:?})"));
            }
        }
    }

    pub(super) fn handle_plugin_content_end(&mut self, name: &str, params: &serde_json::Value) {
        let Some(path) = params
            .get("path")
            .and_then(|value| value.as_str())
            .map(PathBuf::from)
        else {
            return;
        };
        let Some(content_id) = params.get("content_id").and_then(|value| value.as_str()) else {
            return;
        };
        if content_id.is_empty() || content_id.len() > 128 {
            return;
        }
        let Some(total_chunks) = params
            .get("total_chunks")
            .and_then(serde_json::Value::as_u64)
            .and_then(|total| usize::try_from(total).ok())
        else {
            return;
        };

        self.ensure_plugin_content_stream(name, &path, content_id);
        let Some((owner, stream)) = self.plugin_content_streams.get_mut(&path) else {
            return;
        };
        if owner != name {
            return;
        }
        if let Err(error) = stream.finish(total_chunks, Instant::now()) {
            self.plugin_message = Some(format!("[{name}] rendered content incomplete ({error:?})"));
        }
    }

    fn ensure_plugin_content_stream(&mut self, name: &str, path: &Path, id: &str) {
        let is_new = self
            .plugin_content_streams
            .get(path)
            .is_none_or(|(owner, stream)| owner != name || stream.content_id() != id);
        if is_new {
            self.plugin_content.insert(path.to_path_buf(), Vec::new());
            self.plugin_content_text
                .insert(path.to_path_buf(), Vec::new());
            if self.current_file.as_deref() == Some(path) && self.filter_query.is_some() {
                self.filter_display_map.clear();
            }
            self.plugin_content_streams.insert(
                path.to_path_buf(),
                (
                    name.to_string(),
                    crate::plugin::ContentStream::new(id.to_string(), Instant::now()),
                ),
            );
            self.activate_plugin_content(path);
        }
        self.plugin_contributions
            .entry(name.to_string())
            .or_default()
            .content_paths
            .insert(path.to_path_buf());
    }

    fn append_plugin_content_lines(&mut self, name: &str, path: &Path, lines: Vec<String>) {
        if lines.is_empty() {
            return;
        }
        let rendered: Vec<Vec<(ratatui::style::Style, String)>> = lines
            .iter()
            .map(|line| crate::ansi::parse_ansi_line(line))
            .collect();
        let text: Vec<String> = rendered
            .iter()
            .map(|spans| spans.iter().map(|(_, value)| value.as_str()).collect())
            .collect();
        let first_line = self.plugin_content_text.get(path).map_or(0, Vec::len);
        let filter_matches = if self.current_file.as_deref() == Some(path) {
            self.filter_query
                .as_deref()
                .filter(|query| !query.is_empty())
                .map(|query| {
                    let query = query.to_lowercase();
                    text.iter()
                        .enumerate()
                        .filter(|(_, line)| line.to_lowercase().contains(&query))
                        .map(|(index, _)| first_line + index)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        self.plugin_content
            .entry(path.to_path_buf())
            .or_default()
            .extend(rendered);
        self.plugin_content_text
            .entry(path.to_path_buf())
            .or_default()
            .extend(text);
        self.plugin_contributions
            .entry(name.to_string())
            .or_default()
            .content_paths
            .insert(path.to_path_buf());
        if self.current_file.as_deref() == Some(path) {
            self.content_revision = self.content_revision.wrapping_add(1);
            if self.filter_query.is_some() {
                self.filter_display_map.extend(filter_matches);
            }
            self.clamp_content_scroll();
        }
    }

    fn activate_plugin_content(&mut self, path: &Path) {
        if self.current_file.as_deref() != Some(path) {
            return;
        }
        let first_render =
            self.plugin_content_active_path.as_deref() != self.current_file.as_deref();
        self.plugin_content_active_path = self.current_file.clone();
        if first_render {
            self.set_content_scroll(0);
            self.content_hscroll = 0;
        } else {
            self.clamp_content_scroll();
        }
        self.plugin_content_active = true;
    }

    pub(super) fn expire_plugin_content_streams(&mut self, now: Instant) {
        for (name, stream) in self.plugin_content_streams.values_mut() {
            if stream.expire(now) {
                self.plugin_message = Some(format!(
                    "[{name}] rendered content incomplete (stream timed out)"
                ));
            }
        }
    }
}

#[cfg(test)]
#[path = "plugin_content_test.rs"]
mod tests;
