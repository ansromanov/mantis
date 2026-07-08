# Telemetry & Bug Reports

`mantis` ships two local-only diagnostic features: an **opt-in usage
telemetry log** and an on-demand **bug report** command. Neither sends
anything anywhere — both write files to your machine that you can inspect,
delete, or choose to share when filing an issue.

## Bug reports

Open the command palette (`Ctrl+P`) and run **"Report a bug (save
diagnostics locally)"**. This opens a modal dialog where you can write a
description of the bug and preview the diagnostic report below it.

When you submit the report (via `Ctrl+S` or `Ctrl+Enter`), `mantis`:
1. Saves the report as markdown under the state directory
   (`~/.local/state/mantis/bug-reports/` on Linux/macOS,
   `%APPDATA%\mantis\bug-reports\` on Windows).
2. Attempts to open your default browser to create a new GitHub issue pre-filled
   with your description and the diagnostic report.
3. If the report exceeds the URL length limit (~6KB) or if the browser fails to
   open, it copies the full report to your clipboard so you can paste it manually,
   updating the status bar with instructions.

The report contains, in full:

- app version and release date
- OS, architecture, OS version, and whether the session runs under WSL
- terminal identity: the `TERM`, `TERM_PROGRAM`, `TERM_PROGRAM_VERSION`, and
  `COLORTERM` environment variables, terminal size, and booleans for Windows
  Terminal / SSH sessions
- workspace *shape*: visible node/file/directory counts, tree depth, number
  of expanded directories, walk-error count, and whether it is a git repo
- open-file facts: extension, size, line count, encoding, line endings,
  detected syntax name, and JSON/diff/memory-mapped flags — never the name
- which config keys differ from defaults (key paths only, never values),
  the theme name, and the plugin *count*
- whether telemetry is enabled

It deliberately contains **no personal data**: no absolute paths, no file or
directory names, no file content, no config values, no plugin names, and no
environment variables beyond the terminal whitelist above.

## Telemetry

Telemetry is **disabled by default**. Toggle it from the command palette
(`Ctrl+P` → **"Toggle telemetry"**) — the status bar confirms the new state
and the setting persists to `mantis.toml`:

```toml
[telemetry]
enabled = true
```

You can also set this key by hand instead of using the palette.

When enabled, whitelisted events are appended to a local JSONL log under
`<state dir>/telemetry/` (`events.jsonl`, rotated at 1 MiB, at most five
files kept). When disabled — the default — no events are recorded and no
files or threads are created. Data never leaves your machine; there is no
upload endpoint.

### Complete event schema

Events are a closed set; each line also carries `ts_ms`, milliseconds since
the session started (no wall-clock timestamps).

| Event | Fields | Recorded when |
|---|---|---|
| `session_start` | `app_version`, `os`, `arch`, `terminal` (the `$TERM` value) | mantis starts |
| `session_end` | `duration_s`, `events_dropped` | mantis exits |
| `action_invoked` | `action` (canonical action id), `source` (`palette`, `key`, `mouse`) | a command palette, key binding, or mouse action runs |
| `overlay_opened` | `kind` (help, about, theme_picker, command_palette, etc.) | an overlay popup is opened |
| `feature_used` | `feature` (fold, diff_nav, git_history, visual_mode, git_blame) | a core feature is used |
| `plugin_toggled` | `kind` (linter, formatter, etc.), `enabled` (bool) | a plugin is enabled/disabled |
| `file_opened` | `size_bucket` (under_1kb, from_1kb_to_1mb, etc.), `source_kind` (tree, search, history, etc.), `encoding`, `is_binary` | a file is opened in the content pane |
| `perf_span` | `span` (open_file, build_visible, highlight, etc.), `duration_bucket` (<1ms, 1-16ms, 16-100ms, >100ms) | an instrumented hot-path span finishes |
| `error_occurred` | `module`, `kind` (error classification/variant name) | an error event is intercepted |

Raw keystrokes, search queries, palette query text, file names, and paths
are never recorded — the event types above cannot carry them. To stop
collecting, set `enabled = false` (or remove the key); to erase history,
delete the `telemetry/` directory from the state dir.
