//! Bundled Kubernetes manifest awareness plugin for mantis.
//!
//! Per proposal #606: adds schema-aware facts for files that look like
//! Kubernetes manifests (`apiVersion:` + `kind:` present) without touching
//! folding, which stays with the `yaml` provider (#603). Registers the same
//! extensions (`yaml`/`yml`) as the `yaml` plugin but a different capability
//! (`status_facts`), so the two providers coexist on the same extensions —
//! this is the routing the design gap in #606 was waiting on.
//!
//! On `on_file_open`, heuristically scans the file (no full YAML parse — see
//! the epic's "regex/indentation heuristics, not tree-sitter" constraint) for
//! `---`-separated documents that carry both `apiVersion` and `kind` at
//! column 0. For each match it records `kind`, `metadata.name`, and
//! `metadata.namespace`, then reports:
//!
//! - a single resource: its identity, `Kind/name (namespace)`.
//! - multiple resources: the first resource's identity, plus per-kind counts
//!   (`3 Deployments · 2 Services · 1 ConfigMap`), parallel to the per-language
//!   statusbar facts the epic describes for Rust/Python.
//!
//! Per-cursor "which resource is the viewport currently in" (the full
//! breadcrumb vision in #606) needs the host to send cursor/line position on
//! selection change, which the protocol does not carry yet — tracked
//! separately; this plugin reports the first resource in the file instead.

use std::io::{self, BufRead, Write};
use std::path::Path;

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let msg: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let event = msg["event"].as_str().unwrap_or("");
        match event {
            "init" => {
                register_language_provider(&mut stdout.lock());
            }
            "on_file_open" => {
                if let Some(path) = msg["path"].as_str() {
                    handle_open(path, &mut stdout.lock());
                }
            }
            "on_quit" | "shutdown" => break,
            _ => {}
        }
    }
}

fn register_language_provider(out: &mut impl Write) {
    let msg = serde_json::json!({
        "event": "action",
        "action": "register_language_provider",
        "params": {
            "extensions": ["yaml", "yml"],
            "capabilities": ["status_facts"]
        }
    });
    let _ = writeln!(out, "{}", serde_json::to_string(&msg).unwrap());
    let _ = out.flush();
}

fn handle_open(path_str: &str, out: &mut impl Write) {
    let path = Path::new(path_str);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext != "yaml" && ext != "yml" {
        return;
    }
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return,
    };
    let text = format_status_facts(&src);
    let msg = serde_json::json!({
        "event": "action",
        "action": "set_status_facts",
        "params": {
            "path": path_str,
            "text": text
        }
    });
    let _ = writeln!(out, "{}", serde_json::to_string(&msg).unwrap());
    let _ = out.flush();
}

/// A single Kubernetes resource identified in one YAML document: its `kind`,
/// `metadata.name`, and `metadata.namespace`.
#[derive(Debug, PartialEq, Eq, Clone)]
struct K8sResource {
    kind: String,
    name: Option<String>,
    namespace: Option<String>,
}

/// Splits `src` on lines that are exactly `---` (the YAML multi-document
/// separator) and heuristically parses each chunk. Documents that don't carry
/// both `apiVersion` and `kind` at column 0 are not Kubernetes manifests and
/// are silently skipped.
fn parse_k8s_resources(src: &str) -> Vec<K8sResource> {
    split_yaml_docs(src)
        .iter()
        .filter_map(|doc| parse_one_doc(doc))
        .collect()
}

fn split_yaml_docs(src: &str) -> Vec<Vec<&str>> {
    let mut docs: Vec<Vec<&str>> = vec![Vec::new()];
    for line in src.lines() {
        if line.trim_end() == "---" {
            docs.push(Vec::new());
        } else {
            docs.last_mut()
                .expect("docs always has at least one entry")
                .push(line);
        }
    }
    docs
}

/// Scans one document's lines for `apiVersion`/`kind` at column 0, and
/// `name`/`namespace` at whatever indent level `metadata:`'s first child
/// sits at (so a nested `labels.name`, several levels deeper, can't shadow
/// the real `metadata.name`). Requires both `apiVersion` and `kind` to treat
/// the document as a manifest, per #606.
fn parse_one_doc(lines: &[&str]) -> Option<K8sResource> {
    let mut has_api_version = false;
    let mut kind: Option<String> = None;
    let mut name: Option<String> = None;
    let mut namespace: Option<String> = None;
    let mut in_metadata = false;
    let mut metadata_child_indent: Option<usize> = None;

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();

        if indent == 0 {
            in_metadata = trimmed == "metadata:";
            metadata_child_indent = None;
            if let Some(v) = trimmed.strip_prefix("apiVersion:") {
                if !v.trim().is_empty() {
                    has_api_version = true;
                }
            } else if let Some(v) = trimmed.strip_prefix("kind:") {
                let v = unquote(v.trim());
                if !v.is_empty() {
                    kind = Some(v);
                }
            }
            continue;
        }

        if !in_metadata {
            continue;
        }
        let child_indent = *metadata_child_indent.get_or_insert(indent);
        if indent != child_indent {
            continue;
        }
        if name.is_none() {
            if let Some(v) = trimmed.strip_prefix("name:") {
                let v = unquote(v.trim());
                if !v.is_empty() {
                    name = Some(v);
                }
            }
        }
        if namespace.is_none() {
            if let Some(v) = trimmed.strip_prefix("namespace:") {
                let v = unquote(v.trim());
                if !v.is_empty() {
                    namespace = Some(v);
                }
            }
        }
    }

    if !has_api_version || kind.is_none() {
        return None;
    }
    Some(K8sResource {
        kind: kind.expect("checked is_none above"),
        name,
        namespace,
    })
}

/// Strips a single layer of matching `'`/`"` quotes from a scalar value.
fn unquote(s: &str) -> String {
    let bytes = s.as_bytes();
    if bytes.len() >= 2 {
        let (first, last) = (bytes[0], bytes[bytes.len() - 1]);
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return s[1..s.len() - 1].to_string();
        }
    }
    s.to_string()
}

/// Naive English pluralization for Kubernetes `kind` names — good enough for
/// the common built-ins (Deployment, Service, ConfigMap, Ingress,
/// NetworkPolicy, StorageClass, ...) without a dictionary. This is a
/// status-bar hint, not a validator.
fn pluralize(kind: &str) -> String {
    let lower = kind.to_ascii_lowercase();
    if lower.ends_with('s')
        || lower.ends_with('x')
        || lower.ends_with('z')
        || lower.ends_with("ch")
        || lower.ends_with("sh")
    {
        format!("{kind}es")
    } else if lower.ends_with('y')
        && kind.len() >= 2
        && !"aeiouAEIOU".contains(kind.as_bytes()[kind.len() - 2] as char)
    {
        format!("{}ies", &kind[..kind.len() - 1])
    } else {
        format!("{kind}s")
    }
}

fn resource_identity(r: &K8sResource) -> String {
    let name_part = r.name.as_deref().unwrap_or("?");
    match &r.namespace {
        Some(ns) => format!("{}/{name_part} ({ns})", r.kind),
        None => format!("{}/{name_part}", r.kind),
    }
}

/// Builds the `set_status_facts` text for a YAML file. Empty when no document
/// looks like a Kubernetes manifest — the host treats an empty `text` as "no
/// fact", clearing any stale summary left from a previous open of this path.
fn format_status_facts(src: &str) -> String {
    let resources = parse_k8s_resources(src);
    let Some(first) = resources.first() else {
        return String::new();
    };
    let identity = resource_identity(first);
    if resources.len() == 1 {
        return identity;
    }

    let mut counts: Vec<(String, usize)> = Vec::new();
    for r in &resources {
        match counts.iter_mut().find(|(k, _)| k == &r.kind) {
            Some((_, n)) => *n += 1,
            None => counts.push((r.kind.clone(), 1)),
        }
    }
    let counts_str = counts
        .iter()
        .map(|(kind, n)| {
            let label = if *n == 1 {
                kind.clone()
            } else {
                pluralize(kind)
            };
            format!("{n} {label}")
        })
        .collect::<Vec<_>>()
        .join(" \u{b7} ");
    format!("{identity} \u{b7} {counts_str}")
}

#[cfg(test)]
#[path = "main_test.rs"]
mod tests;
