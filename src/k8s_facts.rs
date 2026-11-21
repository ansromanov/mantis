//! Kubernetes manifest awareness for YAML files: a status-bar summary.
//!
//! This is the former bundled `k8s` plugin, now a plain module. It
//! heuristically scans a YAML file (no full parse) for `---`-separated documents
//! that carry both `apiVersion` and `kind` at column 0, recording each one's
//! `kind`, `metadata.name` and `metadata.namespace`. The loader builds a
//! [`K8sSummary`] once per file; the status bar then asks it for the text to show
//! at the cursor:
//!
//! - a single resource: its identity, `Kind/name (namespace)`;
//! - several resources: the one under the cursor plus per-kind counts
//!   (`3 Deployments \u{b7} 2 Services \u{b7} 1 ConfigMap`).
//!
//! Files with no manifest document produce no summary at all.
//!
//! Public items:
//! - [`K8sSummary`]: per-file result with [`K8sSummary::facts_at`].
//! - [`summarize`]: builds a summary, or `None` when the file has no manifests.

/// A single Kubernetes resource identified in one YAML document: its `kind`,
/// `metadata.name`, and `metadata.namespace`.
#[derive(Debug, PartialEq, Eq, Clone)]
struct K8sResource {
    kind: String,
    name: Option<String>,
    namespace: Option<String>,
}

/// One YAML document: its zero-based line range (separators excluded) and, when
/// it is a manifest, the resource it describes.
#[derive(Debug, PartialEq, Eq, Clone)]
struct Document {
    start: usize,
    end: usize,
    resource: Option<K8sResource>,
}

/// Per-file Kubernetes summary, built once on load.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct K8sSummary {
    docs: Vec<Document>,
}

/// Scans `src` and returns a summary, or `None` if no document is a manifest.
pub fn summarize(src: &str) -> Option<K8sSummary> {
    let mut docs = Vec::new();
    let mut start = 0;
    let mut lines: Vec<&str> = Vec::new();
    for (index, line) in src.lines().enumerate() {
        if line.trim_end() == "---" {
            docs.push(Document {
                start,
                end: index,
                resource: parse_one_doc(&lines),
            });
            start = index + 1;
            lines.clear();
        } else {
            lines.push(line);
        }
    }
    docs.push(Document {
        start,
        end: start + lines.len(),
        resource: parse_one_doc(&lines),
    });
    docs.iter()
        .any(|d| d.resource.is_some())
        .then_some(K8sSummary { docs })
}

impl K8sSummary {
    /// The text to show for a one-based cursor line (`None` selects the first
    /// resource). Empty when the cursor is outside any manifest document.
    pub fn facts_at(&self, cursor_line: Option<usize>) -> String {
        let resources: Vec<&K8sResource> = self
            .docs
            .iter()
            .filter_map(|d| d.resource.as_ref())
            .collect();
        let selected = match cursor_line {
            None => resources.first().copied(),
            Some(line) => line.checked_sub(1).and_then(|target| {
                self.docs
                    .iter()
                    .find(|d| (d.start..d.end).contains(&target))
                    .and_then(|d| d.resource.as_ref())
            }),
        };
        let Some(selected) = selected else {
            return String::new();
        };
        let identity = resource_identity(selected);
        if resources.len() == 1 {
            return identity;
        }
        let mut counts: Vec<(&str, usize)> = Vec::new();
        for r in &resources {
            match counts.iter_mut().find(|(k, _)| *k == r.kind) {
                Some((_, n)) => *n += 1,
                None => counts.push((&r.kind, 1)),
            }
        }
        let counts_str = counts
            .iter()
            .map(|(kind, n)| {
                let label = if *n == 1 {
                    (*kind).to_string()
                } else {
                    pluralize(kind)
                };
                format!("{n} {label}")
            })
            .collect::<Vec<_>>()
            .join(" \u{b7} ");
        format!("{identity} \u{b7} {counts_str}")
    }
}

/// Facts for a whole file (first resource), or empty text. Convenience wrapper.
#[cfg(test)]
fn format_status_facts(src: &str) -> String {
    summarize(src).map(|s| s.facts_at(None)).unwrap_or_default()
}

/// Facts for the resource containing a one-based line, or empty text.
#[cfg(test)]
fn format_status_facts_at_line(src: &str, line: usize) -> String {
    summarize(src)
        .map(|s| s.facts_at(Some(line)))
        .unwrap_or_default()
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

#[cfg(test)]
#[path = "k8s_facts_test.rs"]
mod tests;
