# Bug Reports

`mantis` has one local-only diagnostic feature: an on-demand **bug report**
command. It never sends anything anywhere: it writes a file to your machine that
you can inspect, delete, or choose to share when filing an issue. (Earlier
versions also had an opt-in usage-telemetry log; it was removed.)

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
  and the theme name

It deliberately contains **no personal data**: no absolute paths, no file or
directory names, no file content, no config values, and no
environment variables beyond the terminal whitelist above.
