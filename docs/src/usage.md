# Usage & Keybindings

## Basic usage

```sh
tv              # view the current directory
tv path/to/dir  # view a specific directory
tv file.md      # open a single file directly
```

Press `?` at any time for in-app help, and `q` to quit.

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
| `Alt+.`        | Toggle hidden files     |
| `H`            | Git history of current file |
| `b`            | Toggle git blame        |
| `V`            | Visual-line mode (select lines; `b` blames the range) |
| `t`            | Theme picker            |
| `Ctrl+G`       | Toggle git mode (changed files + diffs) |
| `Alt+G`        | Toggle flat / tree view in git mode |

## Tree panel

| Key                  | Action                       |
| -------------------- | ---------------------------- |
| `Up`/`k`, `Down`/`j` | Move selection               |
| `Enter`/`Right`/`l`  | Expand directory / open file |
| `Left`/`h`           | Collapse directory / go up   |
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
| `M`            | Toggle raw / rendered markdown |
| `J`            | Toggle JSON pretty-print     |
| `V`            | Visual-line mode: select lines, `b` blames the range, `Esc` exits |
| `D`            | Toggle side-by-side diff (in a diff) |
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

## JSON pretty-printing

Viewing a JSON file? Press `J` to reformat it with indentation for easier
reading, and `J` again to return to the raw text. Handy for minified `.json`.

## Mouse

- **Click** a tree row to select it — opens a file, or folds/unfolds a
  directory.
- **Click** a pane to focus it.
- **Scroll wheel** scrolls whichever pane is under the cursor.
- In the search and history popups, **single-click** selects an entry and
  **double-click** activates it.
