# Configuration

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
git_mode_toggle = ["ctrl+g"]
git_mode_flat_toggle = ["alt+g"]

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
syntax = "base16-ocean.dark"
```

The `background` role controls panel backgrounds. When set to `reset` (the
default), the terminal's own background shows through. Each preset ships a
preferred background color that you can override or disable with
`transparent_background`. On top of presets, live theme switching instantly
re-applies the current file with the new syntax highlighting theme.
