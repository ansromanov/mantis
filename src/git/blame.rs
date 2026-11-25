//! `git blame` parsing with a small LRU cache, and relative-time formatting.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use super::core::git_cmd;
use super::types::*;

/// Maximum number of files to keep in the blame cache. Blame is only shown
/// for the focused file/selection, so a small cache is sufficient.
const BLAME_CACHE_CAPACITY: usize = 16;

struct CachedBlame {
    mtime: SystemTime,
    lines: Vec<BlameLine>,
}

/// Bounded LRU cache for blame results. Evicts the least-recently-used entry
/// when the cache exceeds `BLAME_CACHE_CAPACITY`.
struct BlameCache {
    map: HashMap<PathBuf, CachedBlame>,
    order: VecDeque<PathBuf>,
}

impl BlameCache {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    fn get(&mut self, path: &Path) -> Option<&CachedBlame> {
        if self.map.contains_key(path) {
            // Move to back (most-recently-used).
            let key = path.to_path_buf();
            if let Some(pos) = self.order.iter().position(|p| p == path) {
                self.order.remove(pos);
                self.order.push_back(key);
            }
            self.map.get(path)
        } else {
            None
        }
    }

    fn insert(&mut self, key: PathBuf, value: CachedBlame) {
        if self.map.contains_key(&key) {
            // Update existing entry and promote.
            if let Some(pos) = self.order.iter().position(|p| *p == key) {
                self.order.remove(pos);
            }
            self.order.push_back(key.clone());
            self.map.insert(key, value);
            return;
        }

        if self.map.len() >= BLAME_CACHE_CAPACITY {
            if let Some(oldest) = self.order.pop_front() {
                self.map.remove(&oldest);
            }
        }

        self.order.push_back(key.clone());
        self.map.insert(key, value);
    }
}

static BLAME_CACHE: OnceLock<Mutex<BlameCache>> = OnceLock::new();

fn blame_cache() -> &'static Mutex<BlameCache> {
    BLAME_CACHE.get_or_init(|| Mutex::new(BlameCache::new()))
}

/// Returns per-line git blame annotations for `file` in the repository at
/// `repo_dir`. Returns an empty `Vec` if the file is untracked, not in a git
/// repo, or git is unavailable.
pub fn file_blame(repo_dir: &Path, file: &Path) -> Vec<BlameLine> {
    let mtime = match std::fs::metadata(file).and_then(|m| m.modified()) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };

    {
        let mut guard = blame_cache().lock().unwrap_or_else(|p| p.into_inner());
        if let Some(cached) = guard.get(file) {
            if cached.mtime == mtime {
                return cached.lines.clone();
            }
        }
    }

    let output = git_cmd()
        .arg("-C")
        .arg(repo_dir)
        .args(["blame", "--porcelain", "--"])
        .arg(file)
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let lines = parse_blame_porcelain(&text);

    {
        let mut guard = blame_cache().lock().unwrap_or_else(|p| p.into_inner());
        guard.insert(
            file.to_path_buf(),
            CachedBlame {
                mtime,
                lines: lines.clone(),
            },
        );
    }

    lines
}

fn parse_blame_porcelain(text: &str) -> Vec<BlameLine> {
    let mut blames = Vec::new();
    let mut meta: HashMap<String, (String, String, String)> = HashMap::new();
    let mut lines = text.lines();

    while let Some(line) = lines.next() {
        let parts: Vec<&str> = line.splitn(4, ' ').collect();
        if parts.len() < 3 {
            continue;
        }
        let hash = parts[0].to_string();
        let line_no: u32 = parts[2].parse().unwrap_or(0);

        let (author, date_relative, subject) = if let Some(m) = meta.get(&hash) {
            (m.0.clone(), m.1.clone(), m.2.clone())
        } else {
            let mut author = String::from("Unknown");
            let mut author_time: u64 = 0;
            let mut subject = String::new();

            loop {
                match lines.next() {
                    Some(meta_line) if meta_line.starts_with("author ") => {
                        author = meta_line["author ".len()..].to_string();
                    }
                    Some(meta_line) if meta_line.starts_with("author-time ") => {
                        author_time = meta_line["author-time ".len()..].parse().unwrap_or(0);
                    }
                    Some(meta_line) if meta_line.starts_with("summary ") => {
                        subject = meta_line["summary ".len()..].to_string();
                    }
                    Some(meta_line) if meta_line.starts_with("filename ") => break,
                    Some(_) => continue,
                    None => break,
                }
            }

            let date_relative = format_relative_time(author_time);
            meta.insert(
                hash.clone(),
                (author.clone(), date_relative.clone(), subject.clone()),
            );
            (author, date_relative, subject)
        };

        if lines.next().is_some() {
            blames.push(BlameLine {
                short_hash: if hash.len() >= 7 {
                    hash[..7].to_string()
                } else {
                    hash.clone()
                },
                commit_hash: hash,
                author,
                date_relative,
                line_no,
                subject,
            });
        }
    }

    blames
}

fn format_relative_time(unix_ts: u64) -> String {
    if unix_ts == 0 {
        return "Not committed yet".to_string();
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let diff = now.saturating_sub(unix_ts);

    if diff < 60 {
        let n = diff.max(1);
        pluralize(n, "second")
    } else if diff < 3600 {
        pluralize(diff / 60, "minute")
    } else if diff < 86400 {
        pluralize(diff / 3600, "hour")
    } else if diff < 604800 {
        pluralize(diff / 86400, "day")
    } else if diff < 2_592_000 {
        pluralize(diff / 604800, "week")
    } else if diff < 31_536_000 {
        pluralize(diff / 2_592_000, "month")
    } else {
        pluralize(diff / 31_536_000, "year")
    }
}

fn pluralize(n: u64, unit: &str) -> String {
    if n == 1 {
        format!("1 {unit} ago")
    } else {
        format!("{n} {unit}s ago")
    }
}

#[cfg(test)]
#[path = "blame_test.rs"]
mod tests;
