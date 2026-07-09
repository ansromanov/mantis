# Mantis Manual Verification Checklist — Windows Terminal (PowerShell)

Use this checklist to manually verify features and keybindings on **Windows Terminal** using **PowerShell**.
Mark items as `[x] Pass`, `[ ] Fail`, or `[ ] N/A` in the `Status` column. If an item fails, detail the issue in the `Notes` column and commit the file so the AI agent can address it.

## System Info
* **OS Version**: Windows 
* **Windows Terminal Version**: 
* **PowerShell Version**: 

---

## 1. Setup & Launch
1. Open Windows Terminal with a PowerShell profile.
2. Navigate to the repository root.
3. Run the launch command:
   ```powershell
   cargo run -- .\e2e\data
   ```

---

## 2. Core Features & Interactions

| Status | Feature | Action to Take | Expected Behavior | Notes |
| :---: | :--- | :--- | :--- | :--- |
| [ ] | **Tree Mouse Click** | Left-click on `rust_sample.rs` in the tree pane. | The file opens in the content pane. | |
| [ ] | **Tree Double Click** | Double-click on any folder/file. | Folders toggle expand/collapse; files open and focus shifts to content. | |
| [ ] | **Tree Scroll Wheel** | Scroll mouse wheel over the tree pane. | The tree viewport scrolls vertically. | |
| [ ] | **Content Mouse Click**| Click inside the content pane. | Focus switches to the content pane (highlighted border). | |
| [ ] | **Content Scroll Wheel**| Scroll mouse wheel over the content pane. | The content viewport scrolls vertically. | |
| [ ] | **Window Resizing** | Resize the terminal window horizontally and vertically. | The panel borders and viewport sizes adjust dynamically without visual bugs. | |
| [ ] | **Pretty Print JSON** | Open `json_sample.json`. | Minified JSON is automatically formatted, syntax-highlighted, and spans multiple lines. | |
| [ ] | **BOM Detection** | Open `bom_utf8_sample.txt`. | Status bar displays `UTF-8 BOM` as the encoding. | |
| [ ] | **CRLF Normalization** | Open `crlf_sample.txt`. | Status bar displays `CRLF` as line ending. Content has no visible `\r` control chars. | |
| [ ] | **Binary Detection** | Open `binary_sample.bin`. | Displays `[binary file — BIN file, 125 B]` and blocks raw text view. | |

---

## 3. Keyboard Shortcuts (Keymap)

| Status | Action | Keybinding | Scope | Expected Behavior | Notes |
| :---: | :--- | :--- | :--- | :--- | :--- |
| [ ] | **Quit** | `Ctrl+C` | Global | Instantly restores terminal and exits. | |
| [ ] | **Quit (Tree)** | `q` | Tree | Restores terminal and exits when tree is focused. | |
| [ ] | **Help Overlay** | `?` or `F1` | Global | Toggles the help keybinding overlay. | |
| [ ] | **Switch Panel** | `Tab` | Global | Toggles focus between Tree and Content pane. | |
| [ ] | **Command Palette** | `Ctrl+P` | Global | Opens the Ctrl+P command palette overlay. | |
| [ ] | **Recent Files** | `Ctrl+O` | Global | Opens the recent files list overlay. | |
| [ ] | **Find Files** | `Ctrl+T` | Global | Opens fuzzy file-name search. | |
| [ ] | **Global Search** | `Ctrl+F` | Global | Opens full-text project search. | |
| [ ] | **Go to Line** | `Ctrl+G` | Content | Opens the line-jump prompt. | |
| [ ] | **Git Mode** | `Ctrl+D` | Global | Toggles git mode (shows changed files in tree). | |
| [ ] | **Toggle Hidden** | `.` | Tree | Toggles visibility of hidden files/directories. | |
| [ ] | **Theme Picker** | `t` | Tree | Opens the theme picker overlay. | |
| [ ] | **File History** | `H` | Tree | Opens git history overlay for selected file. | |
| [ ] | **Plugin Picker** | `p` | Tree | Opens the plugin manager overlay. | |
| [ ] | **Toggle Watcher** | `W` | Tree | Toggles file auto-reload on disk change. | |
| [ ] | **Navigate Up** | `Up` or `k` | Tree/Content | Moves cursor up one line. | |
| [ ] | **Navigate Down** | `Down` or `j` | Tree/Content | Moves cursor down one line. | |
| [ ] | **Expand Folder** | `Right`, `Enter`, `l` | Tree | Expands folder or opens file. | |
| [ ] | **Collapse Folder** | `Left`, `h` | Tree | Collapses active folder or goes to parent. | |
| [ ] | **Collapse All** | `-` | Tree | Collapses all folders in the file tree. | |
| [ ] | **Expand All** | `=` | Tree | Recursively expands all folders. | |
| [ ] | **Parent Directory**| `Backspace` | Tree | Navigates the tree root up one directory level.| |
| [ ] | **Page Up** | `PageUp` | Content | Scrolls content up one page. | |
| [ ] | **Page Down** | `PageDown` | Content | Scrolls content down one page. | |
| [ ] | **Scroll Left** | `Left` | Content | Scrolls content pane horizontally to the left. | |
| [ ] | **Scroll Right** | `Right` | Content | Scrolls content pane horizontally to the right. | |
| [ ] | **Reset Column** | `Home` or `0` | Content | Resets horizontal scroll position to column 0. | |
| [ ] | **Jump to Top** | `g` or `ctrl+Home` | Content | Jumps to the first line of the file. | |
| [ ] | **Jump to Bottom**| `G` or `ctrl+End` | Content | Jumps to the last line of the file. | |
| [ ] | **Fold Toggle** | `Space` | Content | Collapses or expands fold region at cursor. | |
| [ ] | **Toggle Blame** | `Ctrl+B` | Content | Toggles persistent single-line blame on statusbar. | |
| [ ] | **Blame Line** | `B` | Content | Opens Git Blame details popup for the line. | |
| [ ] | **Raw Markdown** | `M` | Content | Toggles rendering raw Markdown file content. | |
| [ ] | **Next Hunk** | `n` | Content | In diff view: jumps cursor to next hunk header. | |
| [ ] | **Prev Hunk** | `N` | Content | In diff view: jumps cursor to prev hunk header. | |
| [ ] | **Copy Line** | `y` | Content | Copies current line text to system clipboard. | |
| [ ] | **Copy File** | `Y` | Content | Copies entire file text to system clipboard. | |
| [ ] | **Copy Path** | `y` | Tree | Copies selected file absolute path. | |
| [ ] | **Copy Rel Path** | `Y` | Tree | Copies selected file relative path. | |

---

## 4. New Features (Since PR #614)

> **Setup note:** the first-run welcome overlay (see below) only shows once
> per state directory. To get a clean run of this section, isolate it first:
> `export MANTIS_STATE_DIR=$(mktemp -d)` (bash/zsh — use an equivalent for
> your shell) before launching `mantis`.

| Status | Feature | Action to Take | Expected Behavior | Notes |
| :---: | :--- | :--- | :--- | :--- |
| [ ] | **Root Clamp (Backspace)** | With `MANTIS_STATE_DIR` set as above, launch `cargo run -- ./e2e/data`. With the tree focused and a top-level item selected, press `Backspace`. | Nothing happens outside the launch directory; status bar shows `Already at root`. | |
| [ ] | **Welcome Overlay** | Launch `mantis` for the first time against the isolated `MANTIS_STATE_DIR`. | A centered popup titled ` Welcome to mantis! ` lists 5 actions (open a file/dir, filter tree by name, find files & run commands, open keybinding help, exit) with live keybinding labels, and a footer `Esc  Dismiss this message`. | |
| [ ] | **Welcome Overlay Dismiss & Persistence** | Press `Esc` to dismiss the welcome overlay, then quit and relaunch `mantis` against the same `MANTIS_STATE_DIR`. | Overlay does not reappear on the second launch. | |
| [ ] | **Command Palette Fuzzy Highlight** | Press `Ctrl+P`, type a few letters of a command (e.g. `bug`). | Matching characters in the command name are bolded in the results list. | |
| [ ] | **Command Palette Context-Aware Dimming** | Open `Ctrl+P` outside a git repository (or on a binary file) and look for `Compare against a revision` / `Blame active line`. | Inapplicable commands render dimmed, sorted below applicable ones, with their description replaced by a reason (e.g. `Blame active line — not in a git repo`); pressing Enter on one does nothing but prints the reason in the status bar. | |
| [ ] | **Compare Mode** | In a git repo, open `Ctrl+P`, run `Compare against a revision`, type `HEAD~1` at the `rev: ` prompt, press Enter. | Tree shows `A`/`M`/`D`/`R` badges for changed files; status bar shows a `[compare: HEAD~1]` badge; selecting a changed file shows its diff. | |
| [ ] | **Exit Compare Mode** | While in compare mode, press `Esc` (or toggle git mode off). | Compare badge and A/M/D/R badges disappear; tree returns to normal. | |
| [ ] | **Blame Pane (full file)** | Open a tracked file in a git repo, focus content pane, press `Ctrl+B`. | A dedicated scrollable blame pane replaces the tree, showing hash/author/date/subject columns per line, synced to the content cursor. Navigate with `j`/`k`/`Up`/`Down`, `g`/`G`, `PgUp`/`PgDown`. | |
| [ ] | **Blame Pane Close** | With the full blame pane open, press `Esc` (or `Ctrl+B` again). | Blame pane closes; tree pane returns. | |
| [ ] | **Single-Line Blame Bar** | With content pane focused on a tracked file, press `B`. | A 2-line bottom bar appears under the tree: line 1 shows `hash  author  date`, line 2 shows the dimmed commit subject. | |
| [ ] | **Bundled Language Plugins** | Press `p` in the tree to open the plugin manager, verify `rust`, `python`, and `go` are enabled (default), close with `Esc`. | Folding (`Space`) works on `.rs`/`.py`/`.go` files. | |
| [ ] | **Icons Hide Tree Arrows** | In the plugin manager, enable `iconize`, close, then look at directory rows in the tree. | Folder rows show an open/closed folder icon glyph instead of a `▶`/`▼` arrow; disabling `iconize` restores the arrows. | |
| [ ] | **Bug Report Modal** | Open `Ctrl+P`, run `Report a bug (save diagnostics locally)`. | A modal titled ` Submit Bug Report ` opens with a multiline `Description (What happened / steps)` text box and a read-only `Diagnostic Payload Preview (Read-Only)` pane below it. | |
| [ ] | **Bug Report Scroll Preview** | With the bug report modal open, use `PgUp`/`PgDown` or the mouse wheel over the diagnostic preview pane. | The read-only diagnostic payload preview scrolls independently of the description box. | |
| [ ] | **Bug Report Submit / Cancel** | Type a short description, press `Ctrl+S` (or `Ctrl+Enter`) to submit; reopen and press `Esc` to cancel instead. | Submit saves a markdown report to `<state dir>/bug-reports/report-<timestamp>.md` (path shown in status bar) and attempts to open a pre-filled GitHub issue in the browser; `Esc` closes the modal with no file written. | |
| [ ] | **Toggle Telemetry** | Open `Ctrl+P`, run `Toggle telemetry` twice. | Status bar shows `telemetry enabled` then `telemetry disabled`. | |
| [ ] | **Editor Config Override / Nano Fallback** | With `$VISUAL`/`$EDITOR` unset and no `[general] editor` configured, focus a file and press `Ctrl+E` (or `e` in the tree). | Opens the file in `nano` if present on `$PATH` (else `vim`/`notepad`); status bar shows `Opened with nano — set $EDITOR to choose your editor` (name varies by fallback). Setting `[general] editor` in `mantis.toml` overrides this without the fallback message. | |
| [ ] | **Shell Completions (CLI)** | Outside the TUI, run `mantis --completions bash` (also try `zsh`, `fish`, `powershell`). | Prints a valid shell completion script to stdout and exits without starting the TUI. | |
| [ ] | **Man Page (CLI)** | Outside the TUI, run `mantis --print-man-page`. | Prints a generated troff man page to stdout and exits without starting the TUI. | |
