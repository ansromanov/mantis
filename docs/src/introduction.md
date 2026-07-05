# Welcome to mantis

**mantis** lets you **browse, read, and review a codebase right in
your terminal** — instantly. Point it at a folder and start moving through your
files with the arrow keys (or your mouse), with syntax highlighting, rendered
markdown, fuzzy search, and git diff/blame/history always one keystroke away.

![mantis screenshot](../media/intro.png)

> 💡 **New here?** You only need two things to get started: the
> [Installation](installation.md) page, then the [Quick Start](quickstart.md).
> Everything else is optional.

## What makes it nice

- ⚡ **Lightweight & instant.** One small binary, no runtime dependencies, and
  nothing to configure before your first run. It opens in milliseconds, even
  over SSH.
- 🌳 **A real tree view.** Navigate folders with the keyboard or mouse,
  respecting your `.gitignore`.
- 🔍 **Fuzzy & full-text search.** Jump to any file by name (`Ctrl+P`), or search
  across the contents of every file (`Ctrl+Shift+F`) — fzf-style, as you type.
- 🎨 **Readable files.** Syntax highlighting for source code, rendered markdown,
  and JSON pretty-printing.
- 🔧 **Git built in.** Per-line blame, working-tree diffs, file history, and
  status-colored tree entries — no plugins required.
- ⌨️ **Discoverable.** Press `?` for help or `Ctrl+Shift+P` for a searchable command
  palette. You don't have to memorize anything.

## Try it in five seconds

```sh
mantis          # open the current directory
mantis path/to/dir  # open a specific directory
mantis file.md      # open a single file directly
```

Press `?` any time for in-app help, and `Ctrl+c` to quit.

## Where to go next

| If you want to… | Read |
| --- | --- |
| Understand when to reach for `mantis` | [Why mantis?](why.md) |
| Get it installed | [Installation](installation.md) |
| Learn the basics in 5 minutes | [Quick Start](quickstart.md) |
| See every key and what it does | [Usage & Keybindings](usage.md) |
| Use blame, diffs, and history | [Git Features](git.md) |
| Tweak themes and keybindings | [Configuration](configuration.md) |
