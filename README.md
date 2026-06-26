# mantis

**Browse, read, and review code in your terminal — instantly.**

**Linux / macOS:**
```sh
curl -fsSL https://raw.githubusercontent.com/ansromanov/mantis/main/install.sh | sh
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/ansromanov/mantis/main/install.ps1 | iex
```

`mantis` is a fast, lightweight file tree viewer with syntax highlighting, markdown
rendering, fuzzy search, code folding, and first-class git tooling (diff, blame,
history). One small binary, no config required — with an optional plugin system
when you want to extend it. Built with [ratatui](https://ratatui.rs).

<p align="center">
  <img src="media/intro.png" alt="mantis" width="800">
</p>

```sh
mantis      # open the current directory and start browsing
```

That's it — no setup step. Press `?` for help, `q` to quit.

## Why mantis?

`mantis` is built for one job and does it well: **moving through a codebase and
reading it** — with git context one keystroke away. It is not a full editor, and
that's the point.

| | **mantis** | **Vim / Neovim** | **VS Code** |
| --- | --- | --- | --- |
| Footprint | Single ~MB binary | Light core, heavy once configured | Electron, hundreds of MB + RAM |
| Setup to be useful | **Zero** — just run `mantis` | Hours of config & plugins | Install, extensions, indexing |
| Git diff/blame/history | **Built in** | Needs fugitive/gitsigns/etc. | Needs extensions |
| Fuzzy + full-text search | **Built in** | Needs telescope/fzf/ripgrep glue | Built in |
| Starts in | Milliseconds | Fast (slower with a big config) | Seconds |

Reach for `mantis` when you want to **explore a repo, read a file, or check a diff**
without spinning up a heavyweight editor. Hit `e` to jump into your `$EDITOR`
the moment you actually need to change something.

## Features

- **Lightweight & instant** — a single small binary, no runtime dependencies or
  config needed to start
- **Tree navigation** with keyboard or mouse, respecting `.gitignore`
- **Fuzzy search** (`/`) over file names, or full-text search (`f`) across file
  contents — fzf-style, as-you-type
- **Git mode** (`Ctrl+G`) — show only changed files with working-tree diffs;
  `Alt+G` toggles between tree and flat list views
- **Git blame** (`b`) — inline per-line author, short hash, and date
- **Visual-line mode** (`V`) — vim-style whole-line selection; press `b` to open
  a blame panel scoped to the selected range
- **Git file history** (`H`) — pick a past revision from an fzf-style list and
  view its diff against your working tree, with red/green coloring
- **Git status indicators** — tree entries colored by git status (new, modified,
  deleted, ignored)
- **Syntax highlighting** via [syntect](https://github.com/trishume/syntect)
- **Code folding** (`Space`) — collapse/expand blocks; per-file-type fold regions
  supplied by language plugins, with built-in YAML indentation folding
- **Markdown rendering** — headings, tables, task lists, code blocks,
  blockquotes, and more (press `M` to toggle the raw source)
- **JSON pretty-printing** (`J`) — reformat minified JSON for readable browsing
- **Go to line** (`:`) — jump straight to a line number
- **Status bar** — active line number, detected language, scroll position, and
  (toggleable) encoding/line endings
- **Command palette** (`Ctrl+P`) — fuzzy-find every action and see its keybinding
- **Plugins** (`p`) — opt-in process and syntax plugins, with a git-backed
  registry; enable/disable from a palette, state persists across restarts
- **Session persistence** — expanded dirs, last open file, scroll position, and
  git mode restored on restart (cached outside the repo)
- **Recent files** (`Ctrl+O`) and **copy path** (`y` / `Y`)
- **Open in your editor** (`e`) — jump to the current file in `$VISUAL`/`$EDITOR`,
  then drop back into `mantis` when you're done
- **Themes** — built-in presets (monokai, solarized, catppuccin, synthwave84),
  switchable live, with configurable panel background and terminal transparency
- **Mouse support** — click to select, fold/unfold directories, switch panes,
  scroll, and double-click a directory to change root
- **Configurable** layout, behavior, and keybindings via a simple TOML file

## Install

The one-liners above (no Rust toolchain required) download the prebuilt binary
for your platform, verify its checksum, and install it onto your `PATH`.
With the Rust toolchain: `cargo install mantis`. Or from source:

```sh
git clone https://github.com/ansromanov/mantis.git
cd mantis && cargo build --release   # binary at target/release/mantis
```

See the [installation docs](https://ansromanov.github.io/mantis/installation.html)
for prebuilt binaries, Windows, and checksum verification.

## Usage

```sh
mantis          # view the current directory
mantis path/to/dir  # view a specific directory
mantis file.md      # open a file directly
```

Press `?` at any time for in-app help, and `q` to quit.

## Keybindings

Press `?` in-app for the full list, or `Ctrl+P` to fuzzy-find any action with its
binding. The essentials:

| Key | Action |
| --- | --- |
| `q`, `Ctrl+c` | Quit |
| `?` | Toggle help |
| `Ctrl+P` · `p` | Command palette · plugin palette |
| `Tab` | Switch panel |
| `/` · `f` | Fuzzy file search · full-text content search |
| `r` · `e` | Reload tree · open current file in `$EDITOR` |
| `Ctrl+O` · `y` · `Y` | Recent files · copy absolute path · copy relative path |
| `Space` · `:` | Toggle fold at cursor · go to line |
| `Alt+.` | Toggle hidden files |
| `H` · `b` · `B` | Git history · toggle git blame · blame the active line |
| `V` | Visual-line mode (select lines; `b` blames the range) |
| `Ctrl+G` · `Alt+G` | Toggle git mode · flat/tree view in git mode |
| `t` | Theme picker |

**Navigation** — `Up`/`k`, `Down`/`j` move or scroll; `Enter`/`Right`/`l` expand
a directory or open a file; `Left`/`h` collapse or go up; `-`/`=` collapse/expand
the whole tree. In the content panel, `PageUp`/`PageDown` page, `g`/`G` jump to
top/bottom, `0` resets horizontal scroll, `z` toggles word wrap, `L` toggles line
numbers, `Space` toggles a fold, `M` toggles raw/rendered markdown, `J` toggles
JSON pretty-print.

**Search popup** — type to filter, `Up`/`Down` to navigate, `Tab` to switch
files ↔ content mode, `Enter` to open, `Esc` to close.

**Mouse** — click a tree row to select (opens a file or folds a directory), click
a pane to focus it, scroll the pane under the cursor. In popups, single-click
selects and double-click activates.

## Git features

- **History** (`H`) — fzf-style list of commits that touched the open file. Type
  to filter, `Enter`/double-click to load that revision's diff against your
  working tree (additions green, deletions red).
- **Blame** (`b`, `B`) — `b` toggles an inline gutter with short hash, author, and
  date per line; `B` blames just the active line. Unavailable while viewing a diff.
- **Visual-line blame** (`V`) — enter visual-line mode, extend the selection with
  `j`/`k` (or `g`/`G`), then press `b` to open a panel showing the short hash,
  author, relative date, and content for every line in the range. `Esc` exits.
- **Git mode** (`Ctrl+G`) — show only files with uncommitted changes; selecting
  one shows its working-tree diff. `Alt+G` toggles between tree and a flat list
  of changed files. Directories with changes auto-expand; diffs refresh on the
  30-second auto-reload tick and on manual `r`.
- **Diff view** — while viewing a diff, `D` toggles side-by-side, `S` toggles
  the staged diff, and `n`/`N` jump to the next/previous hunk.

All require `git` on your `PATH` and a tracked file/repository. Configure via
`mantis.toml`:

```toml
git_mode = false         # start in git mode (default: false)
git_mode_flat = false    # start in flat list view (default: false)
git_status = true        # colour tree entries by git status (default: true)
git_show_deleted = false # show ghost nodes for deleted tracked files (default: false)
```

## Plugins

`mantis` works fully without plugins, but a plugin system is there when you want to
extend it. Two kinds:

- **Process plugins** — standalone executables that hook into app events and send
  actions back over newline-delimited JSON on stdin/stdout. They can add language
  providers (syntax highlighting + per-file-type fold regions), file-tree icons,
  git overlays, and more. A plugin can be any executable — a compiled binary, a
  script, anything that reads stdin and writes stdout.
- **Syntax plugins** — `.sublime-syntax` files loaded into the syntect highlighter
  at startup to add new file types without rebuilding `mantis`.

Press `p` for the plugin palette to enable/disable plugins; the choice persists
across restarts (under `[plugins]` in `mantis.toml`). Bundled plugins auto-register
and install on first enable, and a git-backed registry (`index.json`) lets
`mantis` discover and fetch community plugins.

See the [Plugins guide](https://ansromanov.github.io/mantis/plugins.html),
[Plugin Registry](https://ansromanov.github.io/mantis/plugin-registry.html),
and [Plugin Development](https://ansromanov.github.io/mantis/plugin-development.html)
docs for the full protocol and manifest (`plugin.toml`) format.

## Configuration

`mantis` reads a `mantis.toml` file. It first looks for one in the directory being
viewed (and its ancestors), then falls back to the global config:
`$XDG_CONFIG_HOME/mantis/mantis.toml` (or `~/.config/mantis/mantis.toml`) on Linux/macOS,
`%APPDATA%\mantis\mantis.toml` on Windows. A project-local file overrides
the global one, so a repository can ship its own defaults.

```toml
show_hidden = false       # show dotfiles
ignore_gitignore = false  # show files excluded by .gitignore
tree_width = 28           # tree panel width, as a percent of the terminal
tree_independent_scroll = false  # PageUp/PageDown & Home/End scroll the tree
                                 # viewport without moving the selection
word_wrap = false         # wrap long lines in the content panel
indent_guides = true      # draw indent guides in the tree
icons = false             # Nerd Font file-type icons (icon map from a plugin)
show_file_info = true     # encoding + line endings in the status bar
recent_files_count = 10   # how many entries the recent-files list (Ctrl+O) keeps

# Every keybinding is remappable under [keys]. Each action takes a list of
# key specs — a single character ("q", "?", "0") or a named key (Up, Enter,
# Tab, Esc, PageUp, ...), optionally prefixed with "ctrl+" / "alt+".
[keys]
quit = ["q", "ctrl+c"]
search_files = ["/"]
nav_up = ["Up", "k"]
tree_expand = ["Enter", "Right", "l"]
# ... see `?` in-app or the command palette for every action name.
```

### Theme

Press `t` for an fzf-style picker to switch themes live, or set one in config.
Built-in presets: `default`, `monokai`, `solarized`, `catppuccin`, `synthwave84`.

Configure under a `[theme]` table. `name` selects a preset as the base; each role
then overrides it. A role takes a color name (`cyan`, `lightyellow`, `reset`) or
a hex value (`#aabbcc`); `syntax` is a
[syntect](https://github.com/trishume/syntect) theme name for file contents.
Anything left unset keeps the preset's value. Presets ship their own background;
the `default` theme leaves it transparent. Set `transparent_background = true` to
keep a preset's colors but use your terminal's background.

```toml
[theme]
name = "catppuccin"             # built-in preset to start from
transparent_background = false  # true = use terminal's background instead
background = "reset"            # panel backgrounds (reset = terminal default)
accent = "cyan"                # focused borders, primary highlights
accent_alt = "yellow"          # popup chrome, keys, prompts
dim = "darkgray"               # unfocused borders, gutters, hints, rules
text = "white"                 # emphasized / default text
dir = "blue"                   # directory entries in the tree
file = "reset"                 # file entries in the tree
selection_bg = "darkgray"      # selected row / status bar background
selection_fg = "yellow"        # selected row foreground in popups
heading1 = "lightcyan"         # markdown H1 / table headers
heading2 = "lightyellow"       # markdown H2
heading3 = "lightgreen"        # markdown H3
code = "lightyellow"           # inline code / code blocks
diff_add = "green"             # added lines in a diff
diff_del = "red"               # removed lines in a diff
syntax = "base16-ocean.dark"
```

See [`example.md`](example.md) for a document that exercises the markdown renderer.

## Development

```sh
just build     # debug build
just run .     # run against the current directory
just test      # run the test suite
just clippy    # lint
```

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for how to build,
test, and submit a pull request, plus the branch/commit conventions and what CI
checks. Project conventions in depth live in [AGENTS.md](AGENTS.md).

## License

[GPL-3.0-or-later](LICENSE) © Andrei Romanov
