# Mantis Manual Verification Checklist — Windows Terminal (WSL)

Use this checklist to manually verify features and keybindings on **Windows Terminal** running **WSL**.
Mark items as `[x] Pass`, `[ ] Fail`, or `[ ] N/A` in the `Status` column. If an item fails, detail the issue in the `Notes` column and commit the file so the AI agent can address it.

## System Info
* **WSL Distribution**: (e.g. Ubuntu 22.04)
* **Windows Terminal Version**: 
* **Shell**: (e.g. bash, zsh)

---

## 1. Setup & Launch
1. Open Windows Terminal with a WSL profile.
2. Navigate to the repository root.
3. Run the launch command:
   ```bash
   cargo run -- ./e2e/data
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
| :---: | :--- | :--- | :--- | :--- | :--- |
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
