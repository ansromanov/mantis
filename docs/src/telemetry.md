# Telemetry & Bug Reports

`mantis` ships two local-only diagnostic features: an **opt-in usage
telemetry log** and an on-demand **bug report** command. Neither sends
anything anywhere — both write files to your machine that you can inspect,
delete, or choose to share when filing an issue.

## Bug reports

Open the command palette (`Ctrl+P`) and run **"Report a bug (save
diagnostics locally)"**. This collects an anonymous diagnostic snapshot,
saves it as markdown under the state directory
(`~/.local/state/mantis/bug-reports/` on Linux/macOS,
`%APPDATA%\mantis\bug-reports\` on Windows), and shows the saved path in the
status bar. Review the file, then attach or paste it into a
[GitHub issue](https://github.com/ansromanov/mantis/issues).

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
| `action_invoked` | `action` (canonical action id), `source` (`palette`) | a command palette entry runs |

Raw keystrokes, search queries, palette query text, file names, and paths
are never recorded — the event types above cannot carry them. To stop
collecting, set `enabled = false` (or remove the key); to erase history,
delete the `telemetry/` directory from the state dir.
