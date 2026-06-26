# Configuration

> 💡 **Configuration is optional.** `tv` works great with no config at all. Come
> here when you want to change colors, remap a key, or set a default behavior.

`tv` reads a `tv.toml` file. It first looks for one in the directory being
viewed (and its ancestors), then falls back to the global config at
`$XDG_CONFIG_HOME/tv.toml` (or `~/.config/tv.toml`). A project-local file
overrides the global one, so a repository can ship its own defaults.

## Defaults vs. your config

Configuration has two layers:

- **Built-in defaults** ship inside `tv` and supply every value. You don't have
  to set anything.
- **Your `tv.toml`** overrides only the keys you set; everything else falls
  through to the defaults.

On first run `tv` creates a tiny stub `tv.toml` (just a header comment) next to a
read-only **`tv.default.toml`** in your config directory. `tv.default.toml` lists
every option with comments and is **refreshed on every upgrade**, so it always
documents the current set of options. Your own `tv.toml` is **never modified by an
upgrade** — edit it freely.

When you change a setting at runtime (e.g. switching theme), `tv` saves only the
keys that differ from the defaults back to your `tv.toml`, keeping it small. To
see all available options, open `tv.default.toml`; to change one, copy that line
into your `tv.toml`.

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
indent_guides = true      # draw indentation guide lines (│) in the tree pane
icons = false             # Nerd Font file-type icons (icon map supplied by a plugin)
```

## Keybindings

Every keybinding is remappable. Each action takes a **list** of key specs, so an
action can have several shortcuts. A spec is a single character (`"q"`, `"?"`,
`"0"`) or a named key (`Up`, `Down`, `Left`, `Right`, `Enter`, `Tab`, `Esc`,
`Backspace`, `PageUp`, `PageDown`, `Home`, `End`, `Space`), optionally prefixed
with modifiers: `"ctrl+c"`.

> **No Alt-modified defaults.** The Alt modifier conflicts with terminal-level key
> processing and is unreliable across terminals. `tv` does not ship any default
> `alt+` bindings. Users can still configure them in `tv.toml` at their own risk.

> **Keyboard layouts.** Keybinding specs are written with Latin characters
> (e.g. `"ctrl+p"`). On terminals that support the [kitty keyboard
> protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/) (kitty, WezTerm,
> foot, ghostty, and others), `tv` automatically uses the physical key position
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

## Theme

Press `t` for an fzf-style picker to switch themes live, or set one in config.
Built-in presets: `default`, `monokai`, `solarized`, `catppuccin`,
`synthwave84`.

Configure under a `[theme]` table. `name` selects a preset as the base; each
role then overrides it. A role takes a color name (`cyan`, `lightyellow`,
`reset`) or a hex value (`#aabbcc`); `syntax` is a
[syntect](https://github.com/trishume/syntect) theme name for file contents.
Anything left unset keeps the preset's value.

Presets ship their own background color; the `default` theme leaves it
transparent (the terminal's background). Set `transparent_background = true`
to keep a preset's colors but use your terminal's background instead.

```toml
[theme]
name = "catppuccin"           # built-in preset to start from

# Optional per-role overrides on top of the preset:
transparent_background = false  # true = use terminal's background instead of the preset's
background = "reset"         # panel backgrounds (Reset = terminal default)
accent = "cyan"               # focused borders, primary highlights
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
git_clean = "green"        # statusbar git indicator: clean working tree
git_dirty = "yellow"       # statusbar git indicator: uncommitted changes
git_conflict = "red"       # statusbar git indicator: conflict / detached HEAD
git_progress = "#ff8700"   # statusbar git indicator: rebase / merge in progress
breadcrumb_fg = "cyan"     # breadcrumb path bar foreground (default: accent)
breadcrumb_bg = "reset"    # breadcrumb path bar background (default: background)
syntax = "base16-ocean.dark"
```

The `background` role controls panel backgrounds. When set to `reset` (the
default), the terminal's own background shows through. Each preset ships a
preferred background color that you can override or disable with
`transparent_background`. On top of presets, live theme switching instantly
re-applies the current file with the new syntax highlighting theme.
