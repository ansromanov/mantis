# Usage & Keybindings

## Basic usage

```sh
tv              # view the current directory
tv path/to/dir  # view a specific directory
tv file.md      # open a single file directly
```

Press `?` at any time for in-app help, and `q` to quit.

## Global

| Key            | Action                  |
| -------------- | ----------------------- |
| `q`, `Ctrl+c`  | Quit                    |
| `?`            | Toggle help             |
| `Tab`          | Switch panel            |
| `/`            | Fuzzy file search       |
| `f`            | Content (full-text) search |
| `r`            | Reload tree             |
| `Alt+.`        | Toggle hidden files     |
| `H`            | Git history of current file |
| `t`            | Theme picker            |
| `Ctrl+G`       | Toggle git mode (changed files + diffs) |
| `Alt+G`        | Toggle flat / tree view in git mode |

## Tree panel

| Key                  | Action                       |
| -------------------- | ---------------------------- |
| `Up`/`k`, `Down`/`j` | Move selection               |
| `Enter`/`Right`/`l`  | Expand directory / open file |
| `Left`/`h`           | Collapse directory / go up   |

## Content panel

| Key            | Action                       |
| -------------- | ---------------------------- |
| `Up`/`k`, `Down`/`j` | Scroll                 |
| `PageUp`/`PageDown`  | Page up / down         |
| `g` / `G`      | Jump to top / bottom         |
| `Left`/`Right` | Horizontal scroll (when wrap off) |
| `0`            | Reset horizontal scroll      |
| `z`            | Toggle word wrap             |
| `M`            | Toggle raw / rendered markdown |

## Search popup

| Key       | Action                          |
| --------- | ------------------------------- |
| *(type)*  | Filter results                  |
| `Up`/`Down` | Navigate results              |
| `Tab`     | Switch files ↔ content mode     |
| `Enter`   | Open selected result            |
| `Esc`     | Close search                    |

## Mouse

- **Click** a tree row to select it — opens a file, or folds/unfolds a
  directory.
- **Click** a pane to focus it.
- **Scroll wheel** scrolls whichever pane is under the cursor.
- In the search and history popups, **single-click** selects an entry and
  **double-click** activates it.
