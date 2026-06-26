# Usage & Keybindings

## Basic usage

```sh
tv              # view the current directory
tv path/to/dir  # view a specific directory
tv file.md      # open a single file directly
```

Press `?` at any time for in-app help, and `q` to quit.

## Session persistence

`tv` automatically remembers your workspace state across restarts:
expanded directories, the last open file, scroll position, and git mode.
State is cached outside the project tree (`~/.local/state/tree-viewer/`
or `%APPDATA%\tree-viewer\`) so it survives re-clones and never writes
dotfiles into the repository. To reset the session for a directory, quit
and delete the `sessions.json` file from the state directory.

> 💡 **Can't remember a key?** Press `?` for the help overlay, or `Ctrl+P` to
> open the command palette and search for an action by name — it shows you the
> shortcut too. New to `tv`? Start with the [Quick Start](quickstart.md).

## Global

These work no matter which panel is focused.

| Key            | Action                  |
| -------------- | ----------------------- |
| `q`, `Ctrl+c`  | Quit                    |
| `?`            | Toggle help             |
| `Ctrl+P`       | Command palette (fuzzy-find any action) |
| `Tab`          | Switch panel            |
| `/`            | Fuzzy file search       |
| `f`            | Content (full-text) search |
| `r`            | Reload tree             |
| `e`            | Open current file in `$EDITOR` |
| `y`            | Copy absolute path to clipboard |
| `Y`            | Copy path relative to tree root to clipboard |
| `.`            | Toggle hidden files     |
| `H`            | Git history of current file |
| `Ctrl+O`       | Recent files (jump to a recently opened file) |
| `p`            | Plugin palette (enable/disable plugins) |
| `:`            | Go to line              |
| `b`            | Toggle git blame        |
| `B`            | Blame the active line   |
| `t`            | Theme picker            |
| `Ctrl+G`       | Toggle git mode (changed files + diffs) |
| `F`            | Toggle flat / tree view in git mode |

## Tree panel

| Key                  | Action                       |
| -------------------- | ---------------------------- |
| `Up`/`k`, `Down`/`j` | Move selection               |
| `Enter`/`Right`/`l`  | Expand directory / open file |
| `Left`/`h`           | Collapse directory / go up   |
| `Backspace`          | Go up one directory          |
| `-`/`=`              | Collapse all / expand all    |
| `g`/`Home`, `G`/`End` | Jump to first / last entry  |

## Content panel

| Key            | Action                       |
| -------------- | ---------------------------- |
| `Up`/`k`, `Down`/`j` | Scroll                 |
| `PageUp`/`PageDown`  | Page up / down         |
| `g`/`Home`, `G`/`End` | Jump to top / bottom   |
| `Left`/`Right` | Horizontal scroll (when wrap off) |
| `0`            | Reset horizontal scroll      |
| `z`            | Toggle word wrap             |
| `L`            | Toggle line numbers          |
| `Space`        | Toggle fold at cursor        |
| `:`            | Go to line                   |
| `M`            | Toggle raw / rendered markdown |
| `J`            | Toggle JSON pretty-print     |
| `B`            | Blame the active line        |
| `D`            | Toggle side-by-side diff (in a diff) |
| `S`            | Toggle staged diff (in a diff) |
| `n`/`N`        | Next / previous hunk (in a diff) |

## Search popup

Open with `/` (file names) or `f` (file contents). Just start typing to filter.

| Key       | Action                          |
| --------- | ------------------------------- |
| *(type)*  | Filter results                  |
| `Up`/`Down` | Navigate results              |
| `Tab`     | Switch files / content mode     |
| `Enter`   | Open selected result            |
| `Esc`     | Close search                    |

## Command palette

Press `Ctrl+P` to open a searchable list of **every** action, each shown next to
its current keybinding. Type to fuzzy-filter (e.g. "blame", "theme", "json"),
navigate with `Up`/`Down`, and press `Enter` to run the highlighted command.
It's the fastest way to discover what `tv` can do without memorizing keys.

## Open in your editor

Press `e` with a file open to launch it in your editor. `tv` uses `$VISUAL`,
then `$EDITOR`, falling back to `vim`. The TUI suspends while the editor runs and
resumes when you exit; the file is reloaded afterwards so you see your changes.

> 💡 `$EDITOR` can include arguments — e.g. `export EDITOR="code --wait"` opens
> the file in VS Code and waits for you to close the tab before returning.

## Status bar

The status bar at the bottom of the screen shows context-sensitive information
about the open file:

- **`Ln N`** — the active (highlighted) line number, 1-indexed.
- **`[Language]`** — the detected syntax name from syntect (e.g. `[Rust]`,
  `[Python]`, `[TOML]`). Hidden when the file type is not recognised or when
  viewing a diff.
- **Scroll percentage** — how far through the file the content pane is
  scrolled.
- **Encoding and line endings** — shown when `I` (file info) is toggled on.

## Code folding

Press `Space` to fold or unfold the block at the cursor. A fold gutter appears
in the content pane when foldable regions are detected, and the status bar shows
fold stats. Fold regions come from two sources: a built-in YAML indentation
detector, and language plugins that supply per-file-type regions over the
[plugin protocol](plugins.md). Plugin regions override the built-in output for
their file extension.

## JSON pretty-printing

Viewing a JSON file? Press `J` to reformat it with indentation for easier
reading, and `J` again to return to the raw text. Handy for minified `.json`.

## Mouse

- **Click** a tree row to select it — opens a file, or folds/unfolds a
  directory.
- **Double-click** a directory to make it the new tree root.
- **Click** a pane to focus it.
- **Scroll wheel** scrolls whichever pane is under the cursor.
- In the search and history popups, **single-click** selects an entry and
  **double-click** activates it.
