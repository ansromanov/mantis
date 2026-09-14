# Plugin Development

This page describes how to write both kinds of `mantis` plugins: **process
plugins** (subprocess-based) and **syntax plugins** (`.sublime-syntax` files).
See [Plugins](plugins.md) for how to install and configure plugins.

---

## Plugin manifest (`plugin.toml`)

Every plugin **must** have a `plugin.toml` manifest file in its own subdirectory
of the plugin directory (see [Plugins](plugins.md) for where that is). The
manifest is how `mantis` discovers the plugin and learns its entry point, version,
and other metadata.

### Schema

```toml
name = "git-tools"                   # Required: plugin name (shown in picker)
version = "0.1.0"                    # Required: semver recommended
description = "git diff on open"     # Optional: one-line description
author = "ansromanov"                # Optional: author name/handle
entry = "run.sh"                     # Required: executable relative to this dir
mantis_protocol = "3"                # Required: IPC protocol version (tv_protocol still accepted, see below)
platforms = ["linux", "macos"]       # Optional: OS filter (default: all)
events = ["on_file_open"]            # Optional: events to subscribe to (empty/absent = all, for back-compat)
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
| `mantis_protocol` | Yes | IPC protocol version (`"3"` for the current protocol). Plugins declaring a different version are skipped. The field was named `tv_protocol` through protocol 2 (pre-rename); `mantis_protocol` is the current name and `tv_protocol` remains accepted as an alias — if both are present, `mantis_protocol` wins. New plugins should use `mantis_protocol`. |
| `platforms` | No | OS filter: list of `"linux"`, `"macos"`, `"windows"`. Absent = all. |
| `events` | No | Events this plugin subscribes to; only listed events are sent to it. Empty or absent means all events are sent (back-compat with pre-subscription plugins). |
| `permissions` | No | Permissions the plugin needs (advisory, shown for review). |

### Protocol version

The `mantis_protocol` field (or its `tv_protocol` alias) must match the host's
expected protocol version. Plugins declaring a mismatched version are silently
skipped during discovery. The host protocol version is also sent to each
plugin on the `init` event (see below) so the plugin can verify compatibility
dynamically.

| Version | Release | Changes |
|---|---|---|
| `"1"` | 0.7.x | Initial protocol. Events: init, on_file_open, on_keypress, on_selection_change, on_theme_change, on_quit, shutdown. Actions: show_message, open_file, set_content, set_icon_map. Git features (set_file_statuses, set_blame_data, set_status_bar_git_info) were removed in 0.11.22 — git is now built in only. |
| `"2"` | 0.8.x | Language providers (register_language_provider, set_fold_regions), event subscription (`events` field in manifest), protocol hardening (bounded queues, line caps), `protocol_version` field on init event. `init`/`on_theme_change` additionally carry an optional `colors` object (0.13.x, additive — does not bump this version) with the active theme's actual role colors as `#rrggbb` hex. |
| `"3"` | 0.14.x | Request/response correlation (`request`/`response` events) so the host can ask a plugin for something and match the reply; a `plugin_error` action for reporting failures outside the request/response flow; key-consumption semantics for `on_keypress` (`key_handled` action, host waits up to one tick); `priority` field on `register_language_provider` plus a status-bar warning on conflicting registrations; manifest field renamed `tv_protocol` → `mantis_protocol` (alias kept, see above). As with every prior protocol bump, discovery requires an exact version match: a manifest still declaring `"2"` is silently skipped, not loaded in a reduced-compatibility mode — plugins must declare `"3"` (via `mantis_protocol`, or its `tv_protocol` alias) to be discovered on this host. `highlight` capability remains formally reserved and unimplemented: real syntax highlighting continues to flow through syntax plugins (`.sublime-syntax` + syntect), not language providers. A `status_facts` capability plus `set_status_facts` action (0.18.x, additive — does not bump this version) let a provider push a free-text status-bar summary for a file, gated the same way as `fold`/`set_fold_regions`; the bundled `k8s` plugin uses it to report Kubernetes resource identity and per-kind counts for `.yaml`/`.yml` files without conflicting with the `yaml` plugin's `fold` registration on the same extensions. Optional `filenames` and `shebangs` selectors for `register_language_provider` (0.21.x, additive — see #836) also do not change the protocol version. The `on_content_cursor_change` event (0.21.x, additive — does not bump this version) sends debounced, one-based line and column coordinates to subscribers. Chunked rendered content (`set_content_chunk` + `set_content_end`, 0.21.x) is also additive. The `diagnostics` capability (0.21.x, additive — see #834) uses request/response without changing the protocol version. |

### Discovery

On startup `mantis` scans every subdirectory of the plugin directory for
`plugin.toml`. Each discovered manifest produces a `(name, PluginEntry)` pair
that appears in the plugin picker. **Discovered plugins default to disabled**
— no code runs without explicit user opt-in via the picker or `mantis.toml`.

If a plugin is also declared in `[plugins]` in `mantis.toml`, the explicit config
entry takes precedence (allowing the user to override the entry path, enable
it, or set its kind).

---

## Process plugins

The protocol for subprocess-based plugins.

### Protocol overview

A process plugin is any executable that:

1. Reads newline-delimited JSON objects from **stdin** (events from `mantis`).
2. Writes newline-delimited JSON objects to **stdout** (actions back to `mantis`).
3. Exits cleanly when it receives `shutdown` (or when stdin closes).

`mantis` spawns each plugin as a subprocess with `stdin`, `stdout`, and
`stderr` all piped. A background reader thread drains each plugin's stdout and
a background writer thread handles stdin so the `mantis` event loop never
blocks on plugin I/O. A third background thread drains `stderr`: it keeps the
most recent line in memory and appends sanitized output to a rotating log
file under the state directory (`plugin-logs/<name>.log`, capped at 64 KB).
If the plugin exits unexpectedly, that last line and log path are surfaced in
the "exited unexpectedly" message and as a badge in the plugin picker.

## Events: tv → plugin (stdin)

Each event is one JSON object on a single line. Unknown fields are ignored.

### `init`

Sent once immediately after spawn, before any user interaction. Includes the
host protocol version so the plugin can verify it is compatible.

```json
{
  "event": "init",
  "theme": "default",
  "colors": {
    "heading1": "#5fd7ff", "heading2": "#ffffaf", "heading3": "#afffaf",
    "accent": "#00ffff", "dim": "#767676", "code": "#ffffaf", "text": "#ffffff"
  },
  "protocol_version": "2"
}
```

The `protocol_version` field is present only on `init`. If the value does
not match what the plugin expects, the plugin should exit gracefully or
fall back to a compatible subset of features.

The `colors` field carries the active theme's actual colors for seven roles
(`heading1`, `heading2`, `heading3`, `accent`, `dim`, `code`, `text`) as
`#rrggbb` hex strings, resolved from the theme's real definition — including
custom themes from `mantis.toml`. Plugins should use these directly (e.g. as
truecolor ANSI, `\x1b[38;2;R;G;Bm`) instead of hardcoding a palette per theme
name, so any theme renders correctly without the plugin needing to know it by
name. `colors` may be absent from an older host; fall back to a built-in
default palette in that case.

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

**Key consumption (protocol 3+).** A plugin that has `on_keypress` in its
manifest `events` list may reply with a `key_handled` action to claim the
keypress:

```json
{"event":"action","action":"key_handled","params":{"handled":true}}
```

When at least one subscribed plugin replies `handled: true`, `mantis` waits
up to one tick (~16ms) after dispatching `on_keypress` before deciding
whether to also run its own normal-mode key handling for that key. If any
reply arrives with `handled: true` within that window, `mantis` swallows the
key — no built-in binding fires for it. If multiple subscribed plugins
reply, the first `handled: true` response the host receives within the
window wins and the key is consumed exactly once; a plugin that doesn't
reply within the window is treated as not having handled the key. Plugins
that never send
`key_handled` behave exactly as under protocol 2 — the keypress always falls
through to normal handling.

### `on_selection_change`

Sent when the tree cursor moves to a different entry or the content cursor
moves to a different source line. `path` is absent if the tree is empty. When
the selection refers to an open text file, `line` contains its zero-based
physical source line; tree-only selections omit it.

```json
{"event":"on_selection_change","path":"/absolute/path/to/entry","line":12}
```

### `on_content_cursor_change`

Sent to subscribers after the content cursor has stayed still for 100 ms. A
burst of cursor moves is coalesced into one event for the latest position.
`line` and `column` are one-based. Mantis' content cursor selects a source
line, so `column` is currently always `1` (the start of that line). This event
is separate from `on_selection_change`, whose existing tree and selection
semantics and zero-based `line` field remain unchanged.

```json
{"event":"on_content_cursor_change","path":"/absolute/path/to/file.yaml","line":13,"column":1}
```

### `on_theme_change`

Sent when the user switches themes at runtime (via the theme picker or command
palette). The `theme` field carries the new theme name exactly as configured,
and `colors` carries its resolved colors (same shape as on `init`; see above).

```json
{
  "event": "on_theme_change",
  "theme": "monokai",
  "colors": {
    "heading1": "#5fd7ff", "heading2": "#ffd787", "heading3": "#afd787",
    "accent": "#af87d7", "dim": "#6c6c6c", "code": "#ffd787", "text": "#ffffff"
  }
}
```

### `command` (protocol 3+)

Sent when the user selects one of the plugin's palette commands, menu bar
entries (see the `register_commands` action below), or context-menu items
(see `register_context_items` below). `params.id` is the command's stable `id`
exactly as the plugin registered it. When activated from a context menu,
`params.target` is also provided:

```json
{"event":"command","params":{"id":"my-plugin.format","target":{"kind":"content","path":"/path/to/file.rs","line":42}}}
```

`target.kind` is `"tree_file"`, `"tree_dir"`, `"content"`, `"tab"`, or `"blame"`.
`target.path` is the target path (or current file path) if available.
`target.line` is the 1-based physical line number when applicable (`content` or `blame`), or null.

### `on_quit`

Sent when the user initiates a quit (before `shutdown`). Use this to do any
final work before the process is torn down.

```json
{"event":"on_quit"}
```

### `shutdown`

Sent as the final event. `mantis` closes stdin immediately after sending this.
Exit cleanly in response.

```json
{"event":"shutdown"}
```

## Requests: mantis ⇄ plugin (protocol 3+)

Protocol 2 is one-way and fire-and-forget in both directions: the host emits
events, the plugin emits actions, and neither side can *ask* the other for
something and wait for a specific reply. Protocol 3 adds a correlated
request/response pair on top of the existing event/action stream, used for
capabilities that need an answer to a specific question (e.g. "fold regions
for this file, now") rather than a broadcast the plugin may or may not act on.

**Host → plugin request**, sent on stdin like any other event:

```json
{"event":"request","id":42,"method":"fold_regions","params":{"path":"/absolute/path/to/file"}}
```

**Plugin → host response**, sent on stdout as its own line, alongside (but
distinct from) `action` lines:

```json
{"event":"response","id":42,"result":{"regions":[[0,5],[10,20]]}}
```

or, on failure:

```json
{"event":"response","id":42,"error":{"message":"failed to parse file"}}
```

Rules:

- `id` is chosen by the host per outstanding request and must be echoed back
  unchanged in the response. IDs are not reused while a request is
  outstanding.
- Exactly one of `result` / `error` must be present.
- The host applies a 300 ms timeout to each request (5 seconds in test builds).
  If no response arrives in time, the host treats it as an error and
  does not kill the plugin. Non-diagnostics requests are logged like a
  `plugin_error`; a missed diagnostics response is silent and leaves the file
  without diagnostic UI.
- `request`/`response` is additive to the existing event/action stream, not a
  replacement: `set_fold_regions` pushed unprompted still works exactly as in
  protocol 2 for plugins that don't implement requests. The host only sends
  `request` events to plugins that declared protocol 3 in their manifest.

This is the surface the `hover` and `definition` capabilities can use in the
future. Diagnostics uses the same pair today, gated by the `diagnostics`
capability in `register_language_provider`.

For diagnostics, the host sends a `diagnostics` request with the current file
path whenever that file is opened or reloaded:

```json
{"event":"request","id":43,"method":"diagnostics","params":{"path":"/repo/main.py"}}
```

The plugin responds with a result object containing a `diagnostics` array:

```json
{"event":"response","id":43,"result":{"diagnostics":[
  {"line":4,"column":8,"end_line":4,"end_column":13,
   "severity":"error","message":"Undefined name `value`","source":"ruff:F821"}
]}}
```

Coordinates are zero-based physical source positions; end coordinates are
optional. `severity` is `error`, `warning`, `info`, or `hint`; `message` and
`source` are strings. The host ignores malformed entries, caps accepted
results at 10,000 items, and ignores late replies after a newer reload.

## Actions: plugin → tv (stdout)

Respond with action objects on stdout. Each object must be on a single line.
Lines that are not valid JSON or that lack `"event":"action"` are silently
ignored.

### `show_message`

Displays a message in the `mantis` status bar.

```json
{"event":"action","action":"show_message","params":{"message":"hello from plugin"}}
```

### `plugin_error` (protocol 3+)

Reports a failure that isn't tied to a specific `request`/`response` pair
(for example, a subscription-only plugin that failed to act on a broadcast
event). Distinct from `show_message`: it is recorded in the plugin's
rotating log file (`plugin-logs/<name>.log`) and surfaced with error styling
in the status bar and plugin picker, rather than treated as routine status
text.

```json
{"event":"action","action":"plugin_error","params":{"message":"failed to parse file","context":"on_file_open"}}
```

Fields:
- `message` — human-readable error description.
- `context` — optional free-form string naming the event/method that failed
  (advisory, shown alongside the message).

### `open_file`

Opens a file in the content panel.

```json
{"event":"action","action":"open_file","params":{"path":"/tmp/output.txt"}}
```

### `set_content`

Replaces the content panel with the given lines. Each line is a string that may
contain ANSI escape codes for colour and styling. `mantis` parses the ANSI codes
with its built-in parser and displays them as styled text. Handy for plugins
that generate rich output (e.g. markdown renderers, linters).

```json
{"event":"action","action":"set_content","params":{"lines":["\u001b[32mgreen line\u001b[0m","plain line"]}}
```

For large rendered documents, use `set_content_chunk` followed by
`set_content_end`. Each chunk is a bounded action, so the host can display
early lines while later chunks are still arriving. `content_id` identifies one
render pass and must be unique for each new render of a path. `index` is a
zero-based chunk number; chunks may arrive out of order, and duplicate indexes
are ignored. `set_content_end` is the explicit terminator and reports the total
number of chunks. The host appends contiguous chunks as they become available
and waits for any gaps before marking the stream complete.

```json
{"event":"action","action":"set_content_chunk","params":{"path":"/tmp/report.md","content_id":"render-17","index":0,"lines":["first line","second line"]}}
{"event":"action","action":"set_content_chunk","params":{"path":"/tmp/report.md","content_id":"render-17","index":1,"lines":["third line"]}}
{"event":"action","action":"set_content_end","params":{"path":"/tmp/report.md","content_id":"render-17","total_chunks":2}}
```

The host accepts chunks up to 1 MiB and caps a streamed document at 64 MiB,
1,000,000 lines, and 65,536 chunks. These limits complement the existing
4 MiB stdout-line limit and bounded plugin action queue. A stream that exceeds
a limit, has an invalid terminator, or is idle for ten seconds keeps its
already-delivered lines and is marked **incomplete** in the status bar. A later
render should use a new `content_id`.

### `set_icon_map`

Sets the file-type icon glyphs used in the tree. Requires `icons = true` in `mantis.toml` and a Nerd Font terminal. Keys in `icons` are file extensions (lowercase) or full filenames for extensionless files (e.g. `"dockerfile"`).

```json
{"event":"action","action":"set_icon_map","params":{"dir_open":"","dir_closed":"","fallback":"","icons":{"rs":"","py":"","dockerfile":""}}}
```

Fields:
- `dir_open` — glyph for open directories
- `dir_closed` — glyph for closed directories
- `fallback` — glyph used when no extension key matches
- `icons` — map of extension/filename → glyph

Once icons are active, the tree's `▶`/`▼` expand/collapse arrows are suppressed —
the `dir_open`/`dir_closed` glyph substitutes as the expansion indicator instead.

### `register_commands` (protocol 3+)

Contributes commands to the Ctrl-P command palette. Typically sent once in
response to `init`. Registered commands appear in the palette after the
built-in commands (they participate in fuzzy search but not in frecency
ranking) and are always applicable. When the user selects one, the host sends
a `command` event back to the plugin with the command's `id` (see the events
section above).

```json
{"event":"action","action":"register_commands","params":{"commands":[
  {"id":"my-plugin.do-thing","name":"Do the thing","category":"Plugin","description":"runs the thing"}
]}}
```

Fields per command:
- `id` — stable identifier, echoed back in the `command` event on selection.
  Prefix it with your plugin name to avoid collisions with built-in action ids.
- `name` — display name shown in the palette.
- `category` — optional grouping label shown as a dimmed prefix.
- `description` — optional one-line description shown dim after the name.

Re-registration replaces the plugin's previous command list entirely; sending
an empty `commands` array clears it. Commands registered here also appear under
the **Plugins** dropdown in the menu bar.

### `register_context_items` (protocol 3+)

Contributes items to context menus across the application. Context menu items
are scoped by target kind and optional file extensions, rendered in a dedicated
plugin group below built-in items, and never interleaved with host actions.

```json
{"event":"action","action":"register_context_items","params":{"items":[
  {
    "id": "my-plugin.format",
    "label": "Format document",
    "target": "content",
    "category": "Format",
    "extensions": ["rs", "toml"],
    "weight": 10
  }
]}}
```

Fields per item:
- `id` — stable identifier, dispatched back to the plugin in a `command` event when activated.
- `label` — display label shown in the context menu (truncated to 40 characters).
- `target` — context scope kind: `"tree_file"`, `"tree_dir"`, `"content"`, `"tab"`, or `"blame"`.
- `category` — optional submenu category (items sharing a category are nested into a submenu).
- `extensions` — optional list of file extensions (case-insensitive) the item applies to.
- `weight` — optional sort weight (lower numbers sort first; ties are broken alphabetically by label).

Limits and invariants:
- A plugin may contribute at most 16 context items. Registrations exceeding this limit or containing malformed parameters report a `plugin_error`.
- Re-registration replaces the plugin's previous items; an empty `items` array clears them.
- Selecting an item dispatches a `command` event containing `id` and `target` information (`kind`, `path`, `line`) back to the plugin.

### `register_status_segments` (protocol 3+)

Contributes persistent segments to the status bar. Segments participate in the status bar
layout, elision ladder, and user color customization via `[statusbar]` in `mantis.toml`.

```json
{"event":"action","action":"register_status_segments","params":{"segments":[
  {
    "id": "git-stats",
    "text": "±3",
    "priority": 3,
    "side": "right"
  }
]}}
```

Fields per segment:
- `id` — unique identifier for the segment. Can be referenced in `[statusbar]` `left`/`right` lists and styled in `[statusbar.colors.<id> = "<role>"]`.
- `text` — text content to display (truncated to 40 characters).
- `priority` — optional priority for the elision ladder (integer from 1 to 5, default: 2; 5 is highest priority / last to be dropped on narrow screens).
- `side` — optional alignment preference: `"left"` or `"right"` (default: `"right"`).

Limits and invariants:
- A plugin may contribute at most 8 status segments. Registrations exceeding this limit or containing malformed parameters report a `plugin_error`.
- Re-registration replaces the plugin's previous segments; an empty `segments` array clears them.

## Language providers

A process plugin can declare itself as a **language provider** by responding to
the `init` event with a `register_language_provider` action. This tells `mantis`
which file extensions the plugin handles and what capabilities it provides.
`fold` and `symbols` are implemented via push (`set_fold_regions` and
`set_symbols`); `diagnostics` uses a request/response; `highlight` is formally
reserved (see below). `hover` and `definition` can use the same request
transport later without a protocol break.

### Provider contract

The language provider contract between `mantis` and a plugin follows a strict
lifecycle:

1. **Registration.** Immediately after receiving `init`, the plugin sends
   `register_language_provider` to declare its extensions and capabilities.
   Registration is expected once per plugin — re-registration overwrites
   the previous registration entirely.

2. **File routing.** When the user opens a file whose extension matches a
   registered provider, `mantis` routes relevant events (`on_file_open`,
   `on_selection_change`) and capability-driven state requests to that
   provider. When exactly one provider matches an extension+capability pair,
   it is used. When more than one provider registers the same
   extension+capability (protocol 3+), the registration with the higher
   `priority` wins; ties break by registration order (first registered
   wins). The first time this happens for a given extension+capability pair,
   `mantis` shows a one-time status-bar warning naming both plugins, so the
   conflict isn't silent. Two providers can register the *same extension*
   without conflicting as long as they declare *different* capabilities —
   e.g. the bundled `yaml` plugin owns `fold` for `.yaml`/`.yml` while the
   bundled `k8s` plugin owns `status_facts` for the same extensions. `fold`
  `status_facts`, `symbols`, and `diagnostics` drive backend state (fold
  regions, status-bar text, symbol outlines, and diagnostics, respectively);
  `highlight` is reserved for future use (see below).

3. **Response.** For each declared capability, the plugin should respond to
   file-related events with the corresponding action:
   - `fold` → respond with `set_fold_regions` when a matching file is opened
      (see below).
   - `status_facts` → respond with `set_status_facts` when a matching file is
      opened (see below).
   - `symbols` → respond with `set_symbols` when a matching file is opened
      (see below).
   - `diagnostics` → respond to the host's `diagnostics` request with a
      `diagnostics` result array (see above).
   - `highlight` → reserved for future use.

4. **Lifetime.** Provider registrations persist for the entire plugin session.
   When a plugin exits or is deactivated, its registrations are removed. If
   the plugin sends `set_fold_regions` for a file before it is opened, `mantis`
   caches the regions and applies them when the file is opened later.

5. **Capability gating.** `mantis` enforces capability checks at runtime. For
   example, `set_fold_regions` is only accepted when the sender has a
   registered provider with the `fold` capability for that file's extension.
   Unknown capability strings are silently ignored, so existing providers
   remain compatible with future protocol extensions.

### `register_language_provider`

Sent by the plugin immediately after receiving `init`. Declares the file
extensions, filenames, shebang interpreters, and capabilities the plugin
provides. `mantis` stores the registration and uses it to route display-state
updates for each open file.

```json
{"event":"action","action":"register_language_provider","params":{
  "extensions": ["py", "pyi"],
  "filenames": ["Makefile", "Dockerfile*"],
  "shebangs": ["python3"],
  "capabilities": ["fold"],
  "priority": 10
}}
```

Fields:
- `extensions` — lowercase file extensions (no leading dot) this provider handles.
- `filenames` — optional exact filenames or filename globs this provider handles.
  Globs support `*` (any sequence) and `?` (one character); matching is
  case-sensitive.
- `shebangs` — optional interpreter executable names, case-insensitive, such as
  `bash`, `sh`, or `python3`. For `env` shebangs the host skips common env flags
  and assignments before reading the interpreter name.
- `capabilities` — one or more of `"highlight"`, `"fold"`, `"status_facts"`,
  `"symbols"`, or `"diagnostics"`. Reserved for future use: `"hover"` and
  `"definition"`.
- `priority` — optional signed integer, default `0` (protocol 3+). Used only
  to break ties between providers with the same match specificity and
  capability; higher wins. Match precedence is exact filename, filename glob,
  extension, then shebang. Equal priority keeps the first registered provider.
  Absent on protocol 2 plugins, which are treated as priority `0`.

Shebang detection uses the first line already read by the background file loader
and is cached with the loaded path; the UI thread does not reopen files to route
providers. Binary files do not match shebang registrations. Conflicts on the
same selector and capability produce a one-time status-bar warning.

**On `highlight`:** it remains declared but formally reserved as of protocol
3 — `mantis` accepts the registration and never dispatches anything for it.
Real syntax highlighting is intentionally not routed through language
providers; it flows through syntax plugins (`.sublime-syntax` files loaded
into syntect, see [Syntax plugins](#syntax-plugins) below) or the built-in
highlighter. This was evaluated as part of the #296 capability audit and
re-confirmed for v3: provider-driven highlighting would mean re-deriving
styled spans over IPC per file/edit, which the syntect path already does
locally and faster. Revisit only if a concrete plugin use case needs
highlighting decisions syntect's grammar model can't express.

After registering, `mantis` sends `on_file_open` when a file is opened. The host
accepts capability output only from the provider selected by the precedence and
priority rules above (e.g. `set_fold_regions` for `"fold"`).

### `set_fold_regions`

Provides fold regions for a file. Plugin-supplied regions override the
built-in YAML indentation-based folding (the reference implementation)
for that file. Each region is a `[start_line, end_line]` pair (0-indexed,
inclusive). When the named file is currently open the regions are applied
immediately; when it is not yet open they are cached and applied the next
time the file is opened.

Fold regions from a plugin are only accepted when that plugin wins routing for
the file with the `fold` capability, by exact filename, filename glob,
extension, or shebang. Regions from unregistered or non-winning plugins are
silently discarded.

```json
{"event":"action","action":"set_fold_regions","params":{
  "path": "/absolute/path/to/file",
  "regions": [[0, 5], [10, 20]]
}}
```

### `set_status_facts`

Provides a short, plugin-owned summary string shown in the status bar
alongside the fold-count segment when the named file is the one currently
open — e.g. `3 pubs · 12 fns` for a Rust file, or a Kubernetes resource's
identity (`Deployment/nginx (default)`). Unlike `set_fold_regions`, this
carries free text rather than a structured value, so the host does not
interpret it beyond displaying it.

Status facts from a plugin are only accepted when that plugin wins routing for
the file with the `status_facts` capability. An empty `text` clears any
previously set fact for that path
(useful when a provider detects the file no longer qualifies, e.g. a YAML
file that stopped looking like a Kubernetes manifest after an edit).

```json
{"event":"action","action":"set_status_facts","params":{
  "path": "/absolute/path/to/file",
  "text": "Deployment/nginx (default)"
}}
```

### `set_symbols`

Provides a flat symbol list for a file. Symbols power the Ctrl+P `@` route,
the `O` outline picker, and the scope breadcrumb beside the cursor line.
Line numbers are zero-based; `end_line` is inclusive when supplied. The host
accepts this action only from a provider that declared the `symbols` capability
for the file extension.

```json
{"event":"action","action":"set_symbols","params":{
  "path":"/absolute/path/to/file.rs",
  "symbols":[
    {"name":"Service","kind":"struct","line":0,"end_line":3},
    {"name":"run","kind":"method","line":5,"end_line":8,"parent":"Service"}
  ]
}}
```

## Rules

- **One JSON object per line.** No pretty-printing, no multi-line objects.
- **Stdout is for actions only.** Don't write debug output there — it will break the
  protocol. Write to stderr instead: `mantis` captures it for crash diagnostics (the
  last line and a rotating on-disk log), but does not otherwise act on it.
- **Exit on shutdown.** When stdin closes or you receive `shutdown`, exit.
  Do not loop forever waiting for more input.
- **Idempotent reads.** `mantis` may send multiple `on_file_open` events for the
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
| `set_content_chunk` / `set_content_end` | Stream assembly state and partial `plugin_content` entries for contributed paths |
| `set_icon_map` | `icon_map`, `icons_enabled`, `icon_dir_open/closed`, `icon_fallback` |
| `set_fold_regions` | `plugin_fold_regions` entries for contributed paths; active fold state reset |
| `set_status_facts` | `plugin_status_facts` entries for contributed paths |
| `set_symbols` | `plugin_symbols` entries for contributed paths |
| `response` (`diagnostics`) | Diagnostic entries, line index, status-bar segments, and picker data for contributed paths |
| `register_language_provider` | Provider registration removed |
| `register_commands` | Palette command registrations removed; an open palette listing them is closed |
| `register_context_items` | Context menu item registrations removed; an open context menu is closed |
| `register_status_segments` | Status segment registrations removed |

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
  in `src/app/file_ops.rs`, `src/app/key_handlers/`, and `src/app/navigation.rs`.
  `on_content_cursor_change` is queued from cursor movement and emitted after
  its debounce in `src/app/refresh.rs`.
- Plugin config deserialization lives in `src/config/mod.rs` under the
   `plugins` key.
- Bundled plugins are declared in `BUNDLED_PLUGINS`
   (`src/plugin/install.rs`) as `(name, binary_name)` pairs and built as
   workspace-member Rust crates under `plugins/`. The
   `install_bundled_plugins()` function finds and copies compiled binaries to
   the plugin directory on first run. The `python` plugin is a language-provider
   example: it registers for `py`/`pyi` with `fold` capability at `init` and
   sends `set_fold_regions` on `on_file_open` using the shared
   `mantis::fold_detectors::indent_fold` detector. The `json` plugin follows
   the same shape for `.json` files, using `mantis::fold_detectors::brace_fold_with_brackets`
   (a `brace_fold` variant that also folds `[…]` arrays) run against the same
   pretty-printed text core renders by default, rather than the raw file —
   otherwise fold-region line numbers would not line up with what's on
   screen for minified JSON. The `yaml` plugin follows the same shape for
   `yaml`/`yml` files, using the shared `mantis::fold_detectors::yaml_fold`
   detector — the same algorithm the built-in
   `crate::yaml_fold::detect_fold_regions` re-exports.


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

`mantis` ships with a `terraform.sublime-syntax` file that is automatically
installed to `{plugin_dir}/syntaxes/` on first run. It provides syntax
highlighting for `.tf`, `.tfvars`, and `.hcl` files (Terraform / HCL).

The terraform syntax is **auto-discovered**: because it lives in the
`syntaxes/` directory and is not tied to a `[plugins]` entry, it is loaded
unconditionally. Folding for the same extensions is provided separately by
the bundled `terraform` **process** plugin, which registers the `fold`
capability via `register_language_provider` and answers `on_file_open`
using the shared `mantis::fold_detectors::hcl_brace_fold` detector.

### Architecture notes

- `src/highlight.rs`::`with_extra_syntaxes()` loads extra syntax definitions
  into the syntax set.
- `src/plugin.rs`::`load_extra_syntaxes()` collects syntax plugins from config
  entries and the `syntaxes/` directory.
- Syntax definitions are loaded once at startup and shared between the main
  thread's `Highlighter` and the background loader thread's `Highlighter`.
