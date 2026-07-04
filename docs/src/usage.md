# Usage & Keybindings

## Basic usage

```sh
mantis          # view the current directory
mantis path/to/dir  # view a specific directory
mantis file.md      # open a single file directly
```

Press `?` at any time for in-app help, and `q` to quit.

## Session persistence

`mantis` automatically remembers your workspace state across restarts:
expanded directories, the last open file, scroll position, and git mode.
State is cached outside the project tree (`~/.local/state/mantis/`
or `%APPDATA%\mantis\`) so it survives re-clones and never writes
dotfiles into the repository. Each workspace root gets its own file under
the `sessions/` subdirectory. To reset the session for a directory, quit
and delete its file from the `sessions/` subdirectory in the state directory.

> 💡 **Can't remember a key?** Press `?` for the help overlay, or `Ctrl+P` to
> open the command palette and search for an action by name — it shows you the
> shortcut too. New to `mantis`? Start with the [Quick Start](quickstart.md).

## Global

These work no matter which panel is focused.

| Key            | Action                  |
| -------------- | ----------------------- |
| `q`, `Ctrl+c`  | Quit                    |
| `?`            | Toggle help             |
| `Ctrl+P`       | Command palette (fuzzy-find any action) |
| `Tab`          | Switch panel            |
| `/`            | Tree filter / in-file search |
| `Ctrl+F`       | Global fuzzy file-name picker |
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
| `b`            | Toggle git blame (shows author + commit subject inline) |
| `B`            | Blame the active line   |
| `t`            | Theme picker            |
| `Ctrl+G`       | Toggle git mode (changed files + diffs; `/` and `f` search scope to changed files) |
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

The content pane has a **line cursor** (visible as a highlighted full-width row). Use `Up`/`Down` to move it, then press `B` to blame the highlighted line.

When git blame is toggled on (`b`), a column appears on the left showing the author name and commit subject for each line. Clicking any cell in this column opens the line-blame popup for that line.

| Key            | Action                       |
| -------------- | ---------------------------- |
| `Up`/`k`, `Down`/`j` | Scroll / move line cursor |
| `PageUp`/`PageDown`  | Page up / down         |
| `g`/`Home`, `G`/`End` | Jump to top / bottom   |
| `Left`/`Right` | Horizontal scroll (when wrap off) |
| `0`            | Reset horizontal scroll      |
| `z`            | Toggle word wrap             |
| `L`            | Toggle line numbers          |
| `Space`        | Toggle fold at cursor        |
| `:`            | Go to line                   |
| `J`            | Toggle JSON pretty-print     |
| `B`            | Blame the active line        |
| `D`            | Toggle side-by-side diff (in a diff) |
| `S`            | Cycle diff source: all / staged / unstaged (in a diff) |
| `n`/`N`        | Next / previous hunk (in a diff) |

### Rendered plugin content and line numbers

`mantis` has no built-in markdown renderer; install and enable the `markdown` plugin (`p` in-app, or `[plugins.markdown]` in `mantis.toml`) for rendered Markdown. When a plugin renders a file's content, line numbers are hidden in the gutter. This is by design: rendered content collapses blank lines, strips code fences, and restructures formatting, so rendered-line numbers don't correspond to source-file line numbers.

## Git features

### Tree colors

Files and folders in the tree are colored by their git status:

| Color  | Meaning |
| ------ | ------- |
| Green  | New / untracked |
| Yellow | Modified |
| Red    | Deleted |
| Gray   | Ignored |

A directory takes the color of the most significant change inside it.

### Status bar

The status bar shows a git summary when inside a repository:

```
[branch  +ahead -behind  N changed]
```

### Git mode and diff navigation

| Key       | Action |
| --------- | ------ |
| `Ctrl+G`  | Toggle git mode — show only changed files; opening a file shows its diff |
| `F`       | Toggle flat list / nested tree (git mode only) |
| `D`       | Toggle side-by-side / unified diff |
| `S`       | Cycle diff source: all (vs HEAD) → staged → unstaged |
| `n` / `N` | Jump to next / previous change hunk |
| `B`       | Blame the current line: hash, author, date, summary |
| `H`       | File history — pick a commit to view its diff |

## Search popup

Three search entry points cover different needs:

- **`Ctrl+F`** — global fuzzy file-name picker. Opens the same file-name search
  from either panel, regardless of focus. Use this when you want to jump to any
  file in the project by name.
- **`/`** — context-sensitive: in the tree panel it filters file names inline;
  in the content panel (with a file open) it opens the in-file search bar.
- **`f`** — fuzzy content search across all files (or changed files in git mode).

Open any search popup and just start typing to filter.
In git mode (`Ctrl+G`), searches are automatically scoped to only the changed
files — the popup title shows "(changed files)" to make this visible.

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
It's the fastest way to discover what `mantis` can do without memorizing keys.

## Git mode history

`H` opens the file's git history in both normal and git mode. The diff of a
selected commit stays on screen and won't be replaced by live file-watcher
updates. Press `Esc` or `r` to return to the current file (or the working-tree
diff in git mode).

## Open in your editor

Press `e` with a file open to launch it in your editor. `mantis` uses `$VISUAL`,
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
- **Double-click** a breadcrumb segment to navigate to that directory.
- In the search and history popups, **single-click** selects an entry and
  **double-click** activates it.
