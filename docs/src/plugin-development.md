# Plugin Development

This page describes how to write both kinds of `tv` plugins: **process
plugins** (subprocess-based) and **syntax plugins** (`.sublime-syntax` files).
See [Plugins](plugins.md) for how to install and configure plugins.

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

Sent once immediately after spawn, before any user interaction.

```json
{"event":"init","theme":"default"}
```

### `on_file_open`

Sent when the user opens a file in the content panel.

```json
{"event":"on_file_open","path":"/absolute/path/to/file"}
```

### `on_keypress`

Sent on every keypress, including inside overlays. The `key` field uses
human-readable notation: `"q"`, `"ctrl+c"`, `"Enter"`, `"alt+."`.

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
## Rules

- **One JSON object per line.** No pretty-printing, no multi-line objects.
- **Stdout is for actions only.** Plugin stderr is discarded; write debug output to a log file instead.
- **Exit on shutdown.** When stdin closes or you receive `shutdown`, exit.
  Do not loop forever waiting for more input.
- **Idempotent reads.** `tv` may send multiple `on_file_open` events for the
  same path if the user reopens a file.
- **No blocking.** Actions are drained non-blockingly every tick. Sending many
  actions in rapid succession is fine; they are buffered and processed in order.

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

- `src/plugin.rs` owns the subprocess lifecycle and the background reader
  thread per plugin.
- `PluginManager` collects events into an internal buffer; `App::tick()` drains
  them via `drain_plugin_actions()` in `src/app/refresh.rs`.
- Hook dispatch (`on_file_open`, `on_keypress`, `on_selection_change`) happens
  in `src/app/file_ops.rs`, `src/app/key_handlers/`, and `src/app/navigation.rs`
  respectively.
- Plugin config deserialization lives in `src/config/mod.rs` under the
  `plugins` key.
- Bundled plugins are declared in `BUNDLED_PLUGINS` (`src/plugin.rs`) as
  `(name, binary_name)` pairs and built as workspace-member Rust crates under
  `plugins/`. The `install_bundled_plugins()` function finds and copies
  compiled binaries to the plugin directory on first run.

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
