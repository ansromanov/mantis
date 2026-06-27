# Themes

Press `t` for an fzf-style picker to switch themes live, or set one in config.
Built-in presets: `default`, `monokai`, `solarized`, `catppuccin`, `synthwave84`.

Configure under a `[theme]` table in your `mantis.toml`. `name` selects a preset as
the base; each role then overrides it. A role takes a color name (`cyan`,
`lightyellow`, `reset`) or a hex value (`#aabbcc`); `syntax` is a
[syntect](https://github.com/trishume/syntect) theme name for file contents.
Anything left unset keeps the preset's value.

Presets ship their own background color; the `default` theme leaves it transparent
(the terminal's background). Set `transparent_background = true` to keep a preset's
colors but use your terminal's background instead.

```toml
[theme]
name = "catppuccin"           # built-in preset to start from

# Optional per-role overrides on top of the preset:
transparent_background = false  # true = use terminal's background instead of the preset's
background = "reset"         # panel backgrounds (reset = terminal default)
accent = "cyan"               # focused borders, primary highlights
accent_alt = "yellow"      # popup chrome, keys, prompts
dim = "darkgray"           # unfocused borders, gutters, hints, rules
text = "white"             # emphasized / default text
dir = "blue"               # directory entries in the tree
file = "reset"             # file entries in the tree
selection_bg = "darkgray"  # selected row / status bar background
selection_fg = "yellow"    # selected row foreground in popups
active_line_bg = "#3a5a5a" # active line cursor highlight (default: selection_bg)
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

The `background` role controls panel backgrounds. When set to `reset` (the default),
the terminal's own background shows through. Each preset ships a preferred background
color that you can override or disable with `transparent_background`. On top of
presets, live theme switching instantly re-applies the current file with the new
syntax highlighting theme.
