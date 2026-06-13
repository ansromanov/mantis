# tree-viewer (`tv`)

**A fast terminal file tree viewer** with syntax highlighting, markdown
rendering, fuzzy search, and mouse support. Built with
[ratatui](https://ratatui.rs).

![tree-viewer screenshot](../media/intro.png)

## Features

- **Tree navigation** with keyboard or mouse, respecting `.gitignore`
- **Syntax highlighting** for source files (via
  [syntect](https://github.com/trishume/syntect))
- **Markdown rendering** in the terminal — headings, tables, task lists, code
  blocks, blockquotes, and more (press `M` to toggle the raw source)
- **Fuzzy search** over file names, or full-text search across file contents
- **Git file history** — pick a past revision from an fzf-style list and view
  its diff against your working tree, with red/green coloring
- **Git mode** (`Ctrl+G`) — show only changed files with working-tree diffs in
  the content panel; `Alt+G` toggles between tree and flat list views
- **Git status indicators** — tree entries colored by git status (new, modified,
  deleted, ignored) whenever `git status` data is available
- **Themes** — built-in presets (monokai, solarized, catppuccin, synthwave84),
  switchable live from an fzf-style picker or set in config, with configurable
  panel background and terminal transparency
- **Mouse support** — click to select, fold/unfold directories, switch panes,
  and scroll
- **Configurable** layout, behavior, and keybindings via a simple TOML file

## Quick start

```sh
tv              # view the current directory
tv path/to/dir  # view a specific directory
tv file.md      # open a file directly
```

Press `?` at any time for in-app help, and `q` to quit.

See the [Installation](installation.md) page for platform-specific install
instructions.
