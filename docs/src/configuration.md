# Configuration

> 💡 **Configuration is optional.** `mantis` works great with no config at all. Come
> here when you want to change colors, remap a key, or set a default behavior.

`mantis` reads a `mantis.toml` file. It first looks for one in the directory being
viewed (and its ancestors), then falls back to the global config at
`$XDG_CONFIG_HOME/mantis/mantis.toml` (or `~/.config/mantis/mantis.toml`). A project-local file
overrides the global one, so a repository can ship its own defaults.

## Defaults vs. your config

Configuration has two layers:

- **Built-in defaults** ship inside `mantis` and supply every value. You don't have
  to set anything.
- **Your `mantis.toml`** overrides only the keys you set; everything else falls
  through to the defaults.

On first run `mantis` creates a tiny stub `mantis.toml` (just a header comment) next to a
read-only **`mantis.default.toml`** in your config directory. `mantis.default.toml` lists
every option with comments and is **refreshed on every upgrade**, so it always
documents the current set of options. Your own `mantis.toml` is **never modified by an
upgrade** — edit it freely.

When you change a setting at runtime (e.g. switching theme), `mantis` saves only the
keys that differ from the defaults back to your `mantis.toml`, keeping it small. To
see all available options, open `mantis.default.toml`; to change one, copy that line
into your `mantis.toml`.

## General options

These top-level keys control default behavior:

```toml
show_hidden = false       # show dotfiles
ignore_gitignore = false  # show files excluded by .gitignore
tree_width = 28           # tree panel width, as a percent of the terminal
word_wrap = false         # wrap long lines in the content panel
line_numbers = true       # show the line-number gutter in the content panel
show_file_info = true     # show encoding and line-ending info in the status bar
recent_files_count = 10   # number of recently opened files to remember

# Git status — which working-tree changes appear in the changed-file list
# and the tree status colors:
# git_show_untracked = true   # include untracked (??) files (default: true)
# git_show_ignored   = false  # include ignored (!!) files (default: false)
# git_show_deleted   = false  # show ghost nodes for deleted tracked files (default: false)
indent_guides = true      # draw indentation guide lines (│) in the tree pane
icons = false             # Nerd Font file-type icons (icon map supplied by a plugin)

# diff_mode = "all"       # default git diff source: "all" (vs HEAD), "staged", "unstaged"
```

## Keybindings

Every keybinding is remappable. Each action takes a **list** of key specs, so an
action can have several shortcuts. A spec is a single character (`"q"`, `"?"`,
`"0"`) or a named key (`Up`, `Down`, `Left`, `Right`, `Enter`, `Tab`, `Esc`,
`Backspace`, `PageUp`, `PageDown`, `Home`, `End`, `Space`), optionally prefixed
with modifiers: `"ctrl+c"`.

> **No Alt-modified defaults.** The Alt modifier conflicts with terminal-level key
> processing and is unreliable across terminals. `mantis` does not ship any default
> `alt+` bindings. Users can still configure them in `mantis.toml` at their own risk.

> **Keyboard layouts.** Keybinding specs are written with Latin characters
> (e.g. `"ctrl+p"`). On terminals that support the [kitty keyboard
> protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/) (kitty, WezTerm,
> foot, ghostty, and others), `mantis` automatically uses the physical key position
> instead of the layout-translated character, so `ctrl+p` works correctly even on
> non-Latin layouts (Russian, Hebrew, etc.). Terminals without kitty protocol
> fall back to the logical character — bindings may not trigger as expected on
> non-Latin layouts in those terminals.

```toml
[keys]
quit = ["q", "ctrl+c"]
help = ["?"]
toggle_hidden = ["."]
search_files = ["/"]
search_content = ["f"]
reload = ["r"]
switch_panel = ["Tab"]
file_history = ["H"]
recent_files = ["ctrl+o"]
theme_picker = ["t"]
plugin_picker = ["p"]
command_palette = ["ctrl+p"]
open_in_editor = ["e"]
copy_path = ["y"]
copy_relative_path = ["Y"]
toggle_blame = ["b"]
blame_line = ["B"]
go_to_line = [":"]
git_mode_toggle = ["ctrl+g"]
git_mode_flat_toggle = ["F"]

nav_up = ["Up", "k"]
nav_down = ["Down", "j"]

tree_expand = ["Enter", "Right", "l"]
tree_collapse = ["Left", "h"]
tree_up_dir = ["Backspace"]
tree_collapse_all = ["-"]
tree_expand_all = ["="]

content_left = ["Left"]
content_right = ["Right"]
content_top = ["g", "Home"]
content_bottom = ["G", "End"]
content_page_up = ["PageUp"]
content_page_down = ["PageDown"]
content_reset_col = ["0"]
fold_toggle = ["Space"]
toggle_wrap = ["z"]
toggle_line_numbers = ["L"]
toggle_raw_markdown = ["M"]
toggle_pretty_json = ["J"]
```

## Command palette ranking

When you open the command palette with `ctrl+p` without typing a query, commands
are ranked by recency and frequency rather than shown in a fixed order. The most
recently used command is pinned at the top; the most frequently used commands
follow it. Type any character to switch to the usual fuzzy search, which ignores
this ordering.

Two options control the ranking:

```toml
# palette_pin_recent = true   # pin the last-used command at the top (default: true)
# palette_frequent_count = 3  # how many most-used commands to pin below it; 0 disables (default: 3)
```

Pinned entries are marked with a `★` prefix in the palette. Usage data is
persisted across sessions in the state directory alongside session history.

## Theme

Themes live under a `[theme]` table and have their own page: see
[Themes](themes.md) for the presets, the live picker (`t`), and every role you
can override.
