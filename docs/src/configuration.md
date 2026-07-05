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

## Options

Options are grouped into tables by area. A few general keys sit at the top
level (they must appear before the first `[table]` header); everything else
lives under `[tree]`, `[content]`, `[search]`, or `[git]`.

> **Migrated from flat keys?** Older configs used flat top-level keys
> (`show_hidden`, `tree_width`, `git_status`, …). Those still load — they are
> folded into the grouped tables automatically — but the grouped form below is
> canonical. `mantis.default.toml` is refreshed to the grouped layout on upgrade.

```toml
# top-level (must precede any [table])
recent_files_count = 10      # number of recently opened files to remember
palette_pin_recent = true    # pin the last-used command atop the Ctrl+P palette
palette_frequent_count = 3   # most-used commands pinned below it; 0 disables

[tree]
show_hidden = false          # show dotfiles / hidden entries
width = 28                   # tree panel width in columns
independent_scroll = false   # PageUp/Down scroll the viewport, not the selection
indent_guides = true         # draw indentation guide lines (│)
icons = false                # Nerd Font file-type icons (icon map from a plugin)

[content]
word_wrap = false            # wrap long lines in the content pane
line_numbers = true          # show the line-number gutter
scrollbar = true             # show a scrollbar
scroll_percentage = true     # show scroll-position percentage
watch = false                # auto-reload the open file when it changes on disk
show_file_info = true        # encoding + line-ending info in the status bar

[search]
in_file_search = true        # enable in-file incremental search via `/`
context_lines = 0            # trailing context lines shown after each match
keep_query = false           # restore the last query when reopening search

[git]
status = true                # show git status colours/markers in the tree
show_untracked = true        # include untracked (??) files
show_ignored = false         # include ignored (!!) files
show_deleted = false         # ghost nodes for deleted tracked files
ignore_gitignore = false     # respect .gitignore when listing files

[git.diff]
mode = "all"                 # default diff source: "all" (vs HEAD) | "staged" | "unstaged"
side_by_side = false         # start the diff view in side-by-side layout
```

## Keybindings

Every keybinding is remappable. Each action takes a **list** of key specs, so an
action can have several shortcuts. A spec is a single character (`"q"`, `"?"`,
`"0"`) or a named key (`Up`, `Down`, `Left`, `Right`, `Enter`, `Tab`, `Esc`,
`Backspace`, `PageUp`, `PageDown`, `Home`, `End`, `Space`, `F1`-`F12`),
optionally prefixed with modifiers: `"ctrl+c"`, `"alt+."`, `"cmd+p"` (`cmd` /
`super` / `command` are all accepted, and map to the platform's Cmd/Super key).

> **Panel scoping.** A spec can also carry a `tree:` or `content:` prefix
> (e.g. `"tree:q"`) to restrict it to that panel; unprefixed specs fire
> regardless of focus. The shipped defaults scope single-letter shortcuts to
> the tree panel so the content pane's letter keyspace stays free for future
> editing features — only a small movement set (`j k h l g G 0 n N`) and
> modifier/F-key/named-key combos work as content-pane defaults. You can
> still bind a bare letter to a content-view action yourself; user overrides
> always take effect regardless of scope.

> **Keyboard layouts.** Keybinding specs are written with Latin characters
> (e.g. `"ctrl+p"`). On terminals that support the [kitty keyboard
> protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/) (kitty, WezTerm,
> foot, ghostty, and others), `mantis` automatically uses the physical key position
> instead of the layout-translated character, so `ctrl+p` works correctly even on
> non-Latin layouts (Russian, Hebrew, etc.). Terminals without kitty protocol
> fall back to the logical character — bindings may not trigger as expected on
> non-Latin layouts in those terminals. Terminals without the kitty protocol also
> can't distinguish `ctrl+shift+<letter>` from `ctrl+<letter>` (written below as
> `ctrl+<Uppercase letter>`, e.g. `ctrl+P`); the defaults are chosen so that
> degradation lands on the more frequent action.

Defaults are editor-style (VS Code / Sublime conventions), with vim motions
kept as tree-panel secondaries:

```toml
[keys]
# global
quit = ["ctrl+c", "tree:q"]
help = ["F1", "tree:?"]
command_palette = ["ctrl+P", "tree:P"]
reload = ["ctrl+r", "F5", "tree:r"]
switch_panel = ["Tab"]
toggle_hidden = ["tree:.", "alt+."]
theme_picker = ["tree:t"]
plugin_picker = ["tree:p"]
open_in_editor = ["ctrl+e", "tree:e"]
copy_path = ["tree:y"]
copy_relative_path = ["tree:Y"]
toggle_watch = ["tree:W"]
recent_files = ["ctrl+o"]
file_history = ["tree:H"]
goto_line = ["ctrl+g"]
git_mode_toggle = ["ctrl+G"]
git_mode_flat_toggle = ["tree:F", "alt+g"]

# search
search_files = ["ctrl+f", "tree:/"]
find_files = ["ctrl+p"]
search_content = ["ctrl+F", "tree:f"]

# navigation (shared by tree and content panes)
nav_up = ["Up", "k"]
nav_down = ["Down", "j"]

# tree pane
tree_expand = ["Enter", "Right", "l"]
tree_collapse = ["Left", "h"]
tree_up_dir = ["Backspace"]
tree_collapse_all = ["-"]
tree_expand_all = ["="]
fold_toggle = ["Space"]

# content pane
content_left = ["Left"]
content_right = ["Right"]
content_top = ["ctrl+Home", "g", "tree:Home"]
content_bottom = ["ctrl+End", "G", "tree:End"]
content_page_up = ["PageUp"]
content_page_down = ["PageDown"]
content_reset_col = ["Home", "0"]
# toggle_wrap, toggle_line_numbers, toggle_pretty_json,
# toggle_diff_side_by_side, and toggle_diff_staged have no default binding —
# they're reachable from the command palette (Ctrl+Shift+P); bind them here
# if you'd like a dedicated key.
toggle_blame = ["ctrl+b"]
blame_line = ["ctrl+B"]

# diff view
diff_hunk_next = ["n"]
diff_hunk_prev = ["N"]
```

> **macOS.** `Keymap::default()` layers Cmd-primary bindings for the most
> frequent actions on top of the table above, keeping every `ctrl+` binding
> as a fallback (Terminal.app/iTerm2 intercept most `cmd+` shortcuts before
> `mantis` sees them; kitty/WezTerm/Ghostty forward them): `find_files =
> ["cmd+p", "ctrl+p"]`, `command_palette = ["cmd+P", "ctrl+P", "tree:P"]`,
> `search_content = ["cmd+F", "ctrl+F", "tree:f"]`, `search_files = ["cmd+f",
> "ctrl+f", "tree:/"]`, `reload = ["cmd+r", "ctrl+r", "F5", "tree:r"]`,
> `recent_files = ["cmd+o", "ctrl+o"]`, `content_top = ["cmd+Up",
> "ctrl+Home", "g", "tree:Home"]`, `content_bottom = ["cmd+Down", "ctrl+End",
> "G", "tree:End"]`, `content_reset_col = ["cmd+Left", "Home", "0"]`.
> `goto_line` and `git_mode_toggle` stay on `ctrl` on every platform, matching
> mac VS Code.

## Command palette ranking

When you open the command palette with `ctrl+shift+p` without typing a query, commands
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

## Status bar

Segments in the status bar can be placed on the left or right side. Left
segments render at column 0; right segments are right-anchored as a block with
spaces between. When the terminal is too narrow, low-priority segments are
dropped from both sides.

**Default mode** (both `left` and `right` unset): all segments are visible.
The historical default places `["lnum", "type", "git", "version"]` on the right,
everything else on the left.

**Explicit allowlist mode** (either `left` or `right` set): only the listed
segment ids render, on their configured side, in the order you specify.
Unlisted segments are hidden. Set both to empty lists for an empty bar.

```toml
[statusbar]
# left = ["badges", "scroll", "lnum", "type", "fileinfo", "git", "errors", "folds", "message"]
# right = ["lnum", "type", "git", "version"]
```

Valid ids: `badges` `scroll` `lnum` `type` `fileinfo` `git` `errors`
`folds` `message` `version`. There is no keybinding-hint segment — the `?`/`F1`
help overlay and the command palette are the discovery surfaces for bindings.

## Theme

Themes live under a `[theme]` table and have their own page: see
[Themes](themes.md) for the presets, the live picker (`t`), and every role you
can override.
