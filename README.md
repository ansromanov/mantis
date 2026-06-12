# tree-viewer (`tv`)

A fast terminal file tree viewer with syntax highlighting, markdown rendering,
fuzzy search, and mouse support. Built with [ratatui](https://ratatui.rs).

<p align="center">
  <img src="media/intro.png" alt="tree-viewer" width="800">
</p>

## Features

- **Tree navigation** with keyboard or mouse, respecting `.gitignore`
- **Syntax highlighting** for source files (via [syntect](https://github.com/trishume/syntect))
- **Markdown rendering** in the terminal — headings, tables, task lists, code
  blocks, blockquotes, and more (press `M` to toggle the raw source)
- **Fuzzy search** over file names, or full-text search across file contents
- **Git file history** — pick a past revision from an fzf-style list and view
  its diff against your working tree, with red/green coloring
- **Themes** — built-in presets (monokai, solarized, catppuccin, synthwave84),
  switchable live from an fzf-style picker or set in config
- **Mouse support** — click to select, fold/unfold directories, switch panes,
  and scroll
- **Configurable** layout, behavior, and keybindings via a simple TOML file

## Install

```sh
git clone https://github.com/ansromanov/tree-viewer.git
cd tree-viewer
cargo build --release
# binary is at target/release/tv
```

Or, if you have [`just`](https://github.com/casey/just):

```sh
just install   # builds --release and copies tv to ~/.cargo/bin
```

## Usage

```sh
tv              # view the current directory
tv path/to/dir  # view a specific directory
tv file.md      # open a file directly
```

Press `?` at any time for in-app help, and `q` to quit.

## Keybindings

### Global

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

### Tree panel

| Key                  | Action                       |
| -------------------- | ---------------------------- |
| `Up`/`k`, `Down`/`j` | Move selection               |
| `Enter`/`Right`/`l`  | Expand directory / open file |
| `Left`/`h`           | Collapse directory / go up   |

### Content panel

| Key            | Action                       |
| -------------- | ---------------------------- |
| `Up`/`k`, `Down`/`j` | Scroll                 |
| `PageUp`/`PageDown`  | Page up / down         |
| `g` / `G`      | Jump to top / bottom         |
| `Left`/`Right` | Horizontal scroll (when wrap off) |
| `0`            | Reset horizontal scroll      |
| `z`            | Toggle word wrap             |
| `M`            | Toggle raw / rendered markdown |

### Search popup

| Key       | Action                          |
| --------- | ------------------------------- |
| *(type)*  | Filter results                  |
| `Up`/`Down` | Navigate results              |
| `Tab`     | Switch files ↔ content mode     |
| `Enter`   | Open selected result            |
| `Esc`     | Close search                    |

## Mouse

- **Click** a tree row to select it — opens a file, or folds/unfolds a directory.
- **Click** a pane to focus it.
- **Scroll wheel** scrolls whichever pane is under the cursor.
- In the search and history popups, **single-click** selects an entry and
  **double-click** activates it.

## Git history

With a file open in the content panel, press `H` to open an fzf-style list of
the commits that touched it. Type to fuzzy-filter, navigate with `↑`/`↓`, and
press `Enter` (or double-click) to load the diff of that revision against your
current working tree into the content panel — additions in green, deletions in
red. Requires `git` on your `PATH` and the file to be tracked in a repository.

## Configuration

`tv` reads a `tv.toml` file. It first looks for one in the directory being
viewed (and its ancestors), then falls back to the global config at
`$XDG_CONFIG_HOME/tv.toml` (or `~/.config/tv.toml`). A project-local file
overrides the global one, so a repository can ship its own defaults.

```toml
show_hidden = false       # show dotfiles
ignore_gitignore = false  # show files excluded by .gitignore
tree_width = 28           # tree panel width, as a percent of the terminal
word_wrap = false         # wrap long lines in the content panel

# Every keybinding is remappable. Each action takes a list of key specs.
# A spec is a single character ("q", "?", "0") or a named key (Up, Down,
# Left, Right, Enter, Tab, Esc, Backspace, PageUp, PageDown, Home, End,
# Space), optionally prefixed with modifiers: "ctrl+c", "alt+.".
[keys]
quit = ["q", "ctrl+c"]
help = ["?"]
toggle_hidden = ["alt+."]
search_files = ["/"]
search_content = ["f"]
reload = ["r"]
switch_panel = ["Tab"]
file_history = ["H"]
theme_picker = ["t"]

nav_up = ["Up", "k"]
nav_down = ["Down", "j"]

tree_expand = ["Enter", "Right", "l"]
tree_collapse = ["Left", "h"]

content_left = ["Left"]
content_right = ["Right"]
content_top = ["g"]
content_bottom = ["G"]
content_page_up = ["PageUp"]
content_page_down = ["PageDown"]
content_reset_col = ["0"]
toggle_wrap = ["z"]
toggle_raw_markdown = ["M"]
```

### Theme

Press `t` for an fzf-style picker to switch themes live, or set one in config.
Built-in presets: `default`, `monokai`, `solarized`, `catppuccin`,
`synthwave84`.

Configure under a `[theme]` table. `name` selects a preset as the base; each
role then overrides it. A role takes a color name (`cyan`, `lightyellow`,
`reset`) or a hex value (`#aabbcc`); `syntax` is a
[syntect](https://github.com/trishume/syntect) theme name for file contents.
Anything left unset keeps the preset's value.

```toml
[theme]
name = "catppuccin"        # built-in preset to start from

# Optional per-role overrides on top of the preset:
accent = "cyan"            # focused borders, primary highlights
accent_alt = "yellow"      # popup chrome, keys, prompts
dim = "darkgray"           # unfocused borders, gutters, hints, rules
text = "white"             # emphasized / default text
dir = "blue"               # directory entries in the tree
file = "reset"             # file entries in the tree
selection_bg = "darkgray"  # selected row / status bar background
selection_fg = "yellow"    # selected row foreground in popups
heading1 = "lightcyan"     # markdown H1 / table headers
heading2 = "lightyellow"   # markdown H2
heading3 = "lightgreen"    # markdown H3
code = "lightyellow"       # inline code / code blocks
diff_add = "green"         # added lines in a diff
diff_del = "red"           # removed lines in a diff
syntax = "base16-ocean.dark"
```

See [`example.md`](example.md) for a document that exercises the markdown
renderer.

## Development

```sh
just build     # debug build
just run .     # run against the current directory
just test      # run the test suite
just clippy    # lint
```

## License

[MIT](LICENSE) © Andrei Romanov
