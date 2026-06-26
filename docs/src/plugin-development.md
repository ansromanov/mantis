# Plugin Development

This page describes how to write both kinds of `tv` plugins: **process
plugins** (subprocess-based) and **syntax plugins** (`.sublime-syntax` files).
See [Plugins](plugins.md) for how to install and configure plugins.

---

## Plugin manifest (`plugin.toml`)

Every plugin **must** have a `plugin.toml` manifest file in its own subdirectory
of the plugin directory (see [Plugins](plugins.md) for where that is). The
manifest is how `tv` discovers the plugin and learns its entry point, version,
and other metadata.

### Schema

```toml
name = "git-tools"                   # Required: plugin name (shown in picker)
version = "0.1.0"                    # Required: semver recommended
description = "git diff on open"     # Optional: one-line description
author = "ansromanov"                # Optional: author name/handle
entry = "run.sh"                     # Required: executable relative to this dir
tv_protocol = "2"                    # Required: IPC protocol version
platforms = ["linux", "macos"]       # Optional: OS filter (default: all)
events = ["on_file_open"]            # Optional: handled events (advisory)
permissions = ["run_git"]            # Optional: required permissions (advisory)
```

Fields:

| Field | Required | Description |
|---|---|---|
| `name` | Yes | Human-readable name shown in the plugin picker. |
| `version` | Yes | Plugin version. Semver recommended. |
| `description` | No | One-line description displayed in the picker. |
| `author` | No | Author name or handle. |
| `entry` | Yes | Path to the executable, relative to this manifest's directory. |
| `tv_protocol` | Yes | IPC protocol version (`"2"` for the current protocol). Plugins declaring a different version are skipped. |
| `platforms` | No | OS filter: list of `"linux"`, `"macos"`, `"windows"`. Absent = all. |
| `events` | No | Events the plugin handles (advisory, not enforced). |
| `permissions` | No | Permissions the plugin needs (advisory, shown at install). |

### Protocol version

The `tv_protocol` field must match the host's expected protocol version.
Plugins declaring a mismatched version are silently skipped during discovery.
The host protocol version is also sent to each plugin on the `init` event (see
below) so the plugin can verify compatibility dynamically.

| Version | Release | Changes |
|---|---|---|
| `"1"` | 0.7.x | Initial protocol. Events: init, on_file_open, on_keypress, on_selection_change, on_theme_change, on_quit, shutdown. Actions: show_message, open_file, set_content, set_file_statuses, set_blame_data, set_status_bar_git_info, set_icon_map. |
| `"2"` | 0.8.x | Language providers (register_language_provider, set_fold_regions), event subscription (`events` field in manifest), protocol hardening (bounded queues, line caps), `protocol_version` field on init event. |

### Discovery

On startup `tv` scans every subdirectory of the plugin directory for
`plugin.toml`. Each discovered manifest produces a `(name, PluginEntry)` pair
that appears in the plugin picker. **Discovered plugins default to disabled**
— no code runs without explicit user opt-in via the picker or `tv.toml`.

If a plugin is also declared in `[plugins]` in `tv.toml`, the explicit config
entry takes precedence (allowing the user to override the entry path, enable
it, or set its kind).

---

## Process plugins

The protocol for subprocess-based plugins.

### Protocol overview

A process plugin is any executable that:

1. Reads newline-delimited JSON objects from **stdin** (events from `tv`).
2. Writes newline-delimited JSON objects to **stdout** (actions back to `tv`).
3. Exits cleanly when it receives `shutdown` (or when stdin closes).

`tv` spawns each plugin as a subprocess with `stdin` and `stdout` piped and
`stderr` discarded (redirected to `/dev/null`). A background reader thread
drains each plugin's stdout and a background writer thread handles stdin so the
`tv` event loop never blocks on plugin I/O.

## Events: tv → plugin (stdin)

Each event is one JSON object on a single line. Unknown fields are ignored.

### `init`

Sent once immediately after spawn, before any user interaction. Includes the
host protocol version so the plugin can verify it is compatible.

```json
{"event":"init","theme":"default","protocol_version":"2"}
```

The `protocol_version` field is present only on `init`. If the value does
not match what the plugin expects, the plugin should exit gracefully or
fall back to a compatible subset of features.

### `on_file_open`

Sent when the user opens a file in the content panel.

```json
{"event":"on_file_open","path":"/absolute/path/to/file"}
```

### `on_keypress`

Sent on every keypress, including inside overlays. The `key` field uses
human-readable notation: `"q"`, `"ctrl+c"`, `"Enter"`.

```json
{"event":"on_keypress","key":"ctrl+p"}
```

### `on_selection_change`

Sent when the tree cursor moves to a different entry. `path` is absent if the
tree is empty.

```json
{"event":"on_selection_change","path":"/absolute/path/to/entry"}
```

### `on_theme_change`

Sent when the user switches themes at runtime (via the theme picker or command
palette). The `theme` field carries the new theme name exactly as configured.

```json
{"event":"on_theme_change","theme":"monokai"}
```

### `on_quit`

Sent when the user initiates a quit (before `shutdown`). Use this to do any
final work before the process is torn down.

```json
{"event":"on_quit"}
```

### `shutdown`

Sent as the final event. `tv` closes stdin immediately after sending this.
Exit cleanly in response.

```json
{"event":"shutdown"}
```

## Actions: plugin → tv (stdout)

Respond with action objects on stdout. Each object must be on a single line.
Lines that are not valid JSON or that lack `"event":"action"` are silently
ignored.

### `show_message`

Displays a message in the `tv` status bar.

```json
{"event":"action","action":"show_message","params":{"message":"hello from plugin"}}
```

### `open_file`

Opens a file in the content panel.

```json
{"event":"action","action":"open_file","params":{"path":"/tmp/output.txt"}}
```

### `set_content`

Replaces the content panel with the given lines. Each line is a string that may
contain ANSI escape codes for colour and styling. `tv` parses the ANSI codes
with its built-in parser and displays them as styled text. Handy for plugins
that generate rich output (e.g. markdown renderers, linters).

```json
{"event":"action","action":"set_content","params":{"lines":["\u001b[32mgreen line\u001b[0m","plain line"]}}
```

### `set_file_statuses`

Provides per-path git status information for tree coloring. The `params` object
maps absolute file paths to status strings. Sent by the bundled `git-plugin` on
`init` and `on_file_open` / `on_selection_change`.

```json
{"event":"action","action":"set_file_statuses","params":{
  "/home/user/proj/src/main.rs": "modified",
  "/home/user/proj/src/lib.rs": "added"
}}
```

Status values: `"modified"`, `"renamed"`, `"conflict"`, `"added"`, `"untracked"`,
`"deleted"`, `"ignored"`.

### `set_blame_data`

Provides per-line blame annotations for a file. Each entry in `lines` is a
pre-formatted display string (hash + author + date) for the corresponding line
(0-indexed). Sent by the bundled `git-plugin` on `b` keypress. When set, these
annotations take precedence over the built-in `git::file_blame()` call in the
content pane.

```json
{"event":"action","action":"set_blame_data","params":{
  "path": "/home/user/proj/src/main.rs",
  "lines": ["abc1234 (John Doe 2024-01-15) ", ...]
}}
```

### `set_status_bar_git_info`

Provides branch, HEAD, and dirty state for the status bar. When set, this takes
precedence over the built-in `git_info`. Sent by the bundled `git-plugin` on
`init` and `on_file_open`. The `state` field is one of `"clean"`, `"dirty"`,
`"conflict"`, `"rebase"`, or `"merge"`.

```json
{"event":"action","action":"set_status_bar_git_info","params":{
  "branch": "main",
  "head": "abc1234",
  "dirty": true,
  "state": "dirty"
}}
```

### `set_icon_map`

Sets the file-type icon glyphs used in the tree. Requires `icons = true` in `tv.toml` and a Nerd Font terminal. Keys in `icons` are file extensions (lowercase) or full filenames for extensionless files (e.g. `"dockerfile"`).

```json
{"event":"action","action":"set_icon_map","params":{"dir_open":"","dir_closed":"","fallback":"","icons":{"rs":"","py":"","dockerfile":""}}}
```

Fields:
- `dir_open` — glyph for open directories
- `dir_closed` — glyph for closed directories
- `fallback` — glyph used when no extension key matches
- `icons` — map of extension/filename → glyph

## Language providers

A process plugin can declare itself as a **language provider** by responding to
the `init` event with a `register_language_provider` action. This tells `tv`
which file extensions the plugin handles and what capabilities it provides.
Both `highlight` and `fold` flow through a single provider protocol; future
capabilities (`hover`, `diagnostics`, `definition`) will slot into the same
surface in a later release without any protocol break.

### Provider contract

The language provider contract between `tv` and a plugin follows a strict
lifecycle:

1. **Registration.** Immediately after receiving `init`, the plugin sends
   `register_language_provider` to declare its extensions and capabilities.
   Registration is expected once per plugin — re-registration overwrites
   the previous registration entirely.

2. **File routing.** When the user opens a file whose extension matches a
   registered provider, `tv` routes relevant events (`on_file_open`,
   `on_selection_change`) and capability-driven state requests to that
   provider. Only the first provider whose extensions match the file's
   extension receives these events for that file. Currently only `fold`
   capability drives backend state (fold regions); `highlight` is reserved
   for future use.

3. **Response.** For each declared capability, the plugin should respond to
   file-related events with the corresponding action:
   - `fold` → respond with `set_fold_regions` when a matching file is opened
      (see below).
   - `highlight` → reserved for future use.

4. **Lifetime.** Provider registrations persist for the entire plugin session.
   When a plugin exits or is deactivated, its registrations are removed. If
   the plugin sends `set_fold_regions` for a file before it is opened, `tv`
   caches the regions and applies them when the file is opened later.

5. **Capability gating.** `tv` enforces capability checks at runtime. For
   example, `set_fold_regions` is only accepted when the sender has a
   registered provider with the `fold` capability for that file's extension.
   Unknown capability strings are silently ignored, so existing providers
   remain compatible with future protocol extensions.

### `register_language_provider`

Sent by the plugin immediately after receiving `init`. Declares the file
extensions and capabilities the plugin provides. `tv` stores the registration
and uses it to route the correct events and display-state updates for each open
file.

```json
{"event":"action","action":"register_language_provider","params":{
  "extensions": ["py", "pyi"],
  "capabilities": ["fold"]
}}
```

Fields:
- `extensions` — lowercase file extensions (no leading dot) this provider handles.
- `capabilities` — one or more of `"highlight"` or `"fold"`. Reserved for
  future use: `"hover"`, `"diagnostics"`, `"definition"`.

After registering, `tv` sends `on_file_open` whenever a matching file is
opened. The plugin should respond with the appropriate action for each declared
capability (e.g. `set_fold_regions` for `"fold"`).

### `set_fold_regions`

Provides fold regions for a file. Plugin-supplied regions override the
built-in YAML indentation-based folding (the reference implementation)
for that file. Each region is a `[start_line, end_line]` pair (0-indexed,
inclusive). When the named file is currently open the regions are applied
immediately; when it is not yet open they are cached and applied the next
time the file is opened.

Fold regions from a plugin are only accepted when `tv` has a registered
language provider with the `fold` capability for the file's extension.
Regions from unregistered plugins or for unmatched extensions are silently
discarded.

```json
{"event":"action","action":"set_fold_regions","params":{
  "path": "/absolute/path/to/file",
  "regions": [[0, 5], [10, 20]]
}}
```

## Rules

- **One JSON object per line.** No pretty-printing, no multi-line objects.
- **Stdout is for actions only.** Plugin stderr is discarded; write debug output to a log file instead.
- **Exit on shutdown.** When stdin closes or you receive `shutdown`, exit.
  Do not loop forever waiting for more input.
- **Idempotent reads.** `tv` may send multiple `on_file_open` events for the
  same path if the user reopens a file.
- **No blocking.** Actions are drained non-blockingly every tick. Sending many
  actions in rapid succession is fine; they are buffered and processed in order.

## State teardown contract

Every `set_*` action a plugin sends **registers a contribution** in the host's
`plugin_contributions` map (`HashMap<String, PluginContributions>`). When the
plugin is disabled via the plugin picker or its process exits unexpectedly,
the host automatically tears down all state that plugin produced — no per-plugin
special cases needed.

**What gets torn down:**

| Action | State cleared |
|---|---|
| `set_content` | `plugin_content` / `plugin_content_text` entries for contributed paths |
| `set_blame_data` | `plugin_blame` entries for contributed paths |
| `set_file_statuses` | `git_status_map` entries for contributed paths |
| `set_status_bar_git_info` | `plugin_git_info` (status bar override) |
| `set_icon_map` | `icon_map`, `icons_enabled`, `icon_dir_open/closed`, `icon_fallback` |
| `set_fold_regions` | `plugin_fold_regions` entries for contributed paths; active fold state reset |
| `register_language_provider` | Provider registration removed |

After clearing, if the disabled plugin had rendered content for the current
file, the file is reloaded from disk and falls back to core rendering
(markdown, JSON pretty-print, or plain-text with syntax highlighting).

**Plugin authors:** There is nothing you need to do to opt in — registration
is automatic. If you write a new `set_*` action in the host code, you must
also stamp it in `PluginContributions` (in `src/plugin/types.rs`) and handle
its teardown in `App::teardown_plugin_contributions` (in `src/app/mod.rs`).
Without this, the disabled plugin's output would persist on screen,
violating the invariant: **disabled plugin = zero observable effect**.

## Minimal Rust example

All bundled plugins are Rust crates under `plugins/`. The simplest possible
plugin responds to `init` with a status message:

```rust
use std::io::{self, BufRead, Write};

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
        match msg["event"].as_str().unwrap_or("") {
            "init" => {
                let response = serde_json::json!({
                    "event": "action",
                    "action": "show_message",
                    "params": {"message": "hello from plugin"}
                });
                let _ = writeln!(stdout.lock(), "{}",
                    serde_json::to_string(&response).unwrap());
            }
            "shutdown" => break,
            _ => {}
        }
    }
}
```

## Architecture notes

- `src/plugin/` owns the subprocess lifecycle, the background reader thread per
  plugin, manifest parsing, binary install, and syntax discovery.
- `PluginManager` (`src/plugin/manager.rs`) collects plugin actions into an
  internal buffer; `App::tick()` drains them via `drain_plugin_actions()` in
  `src/app/refresh.rs`.
- Hook dispatch (`on_file_open`, `on_keypress`, `on_selection_change`) happens
  in `src/app/file_ops.rs`, `src/app/key_handlers/`, and `src/app/navigation.rs`
  respectively.
- Plugin config deserialization lives in `src/config/mod.rs` under the
  `plugins` key.
- Bundled plugins are declared in `BUNDLED_PLUGINS`
  (`src/plugin/install.rs`) as `(name, binary_name)` pairs and built as
  workspace-member Rust crates under `plugins/`. The
  `install_bundled_plugins()` function finds and copies compiled binaries to
  the plugin directory on first run.

---

## Syntax plugins

Syntax plugins provide `.sublime-syntax` files that extend the built-in
syntect-based highlighter. No subprocess is spawned.

### How they work

A syntax plugin is simply a `.sublime-syntax` file (Sublime Text syntax
definition format, YAML-based). At startup:

1. Files in `{plugin_dir}/syntaxes/` are auto-discovered and loaded.
2. Explicit `[plugins]` entries with `kind = "syntax"` are also loaded.
3. Each syntax definition is added to the `SyntaxSet` so syntect associates
   its declared file extensions with highlights.

### Writing a syntax definition

The `.sublime-syntax` format is documented in the [Sublime Text
docs](https://www.sublimetext.com/docs/syntax.html).  A minimal syntax file
looks like:

```yaml
%YAML 1.2
---
name: My Language
file_extensions: [ext1, ext2]
scope: source.my_lang

contexts:
  main:
    - match: '#.*'
      scope: comment.line.number-sign.my_lang
    - match: '\b(keyword)\b'
      scope: keyword.control.my_lang
```

The `file_extensions` key tells syntect which files to highlight with this
syntax. The `scope` key defines the base scope for highlighting, and `contexts`
define the matching rules.

### Bundled syntax plugins

`tv` ships with a `terraform.sublime-syntax` file that is automatically
installed to `{plugin_dir}/syntaxes/` on first run. It provides syntax
highlighting for `.tf` and `.tfvars` files (Terraform / HCL). Enable it by:

```toml
[plugins]
terraform = { kind = "syntax", syntax_file = "syntaxes/terraform.sublime-syntax",
              extensions = ["tf", "tfvars"] }
```

Or simply leave it in the `syntaxes/` directory for auto-discovery (no config
entry needed).

### Architecture notes

- `src/highlight.rs`::`with_extra_syntaxes()` loads extra syntax definitions
  into the syntax set.
- `src/plugin.rs`::`load_extra_syntaxes()` collects syntax plugins from config
  entries and the `syntaxes/` directory.
- Syntax definitions are loaded once at startup and shared between the main
  thread's `Highlighter` and the background loader thread's `Highlighter`.
