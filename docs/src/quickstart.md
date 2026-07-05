# Quick Start

This page walks you through your first few minutes with `mantis`. No prior terminal
expertise required — if you can run one command and press arrow keys, you're set.

> 📦 Don't have it yet? See [Installation](installation.md) first, then come back.

## 1. Open something

Open the folder you're currently in:

```sh
mantis
```

Or point it at a folder or a single file:

```sh
mantis ~/projects/my-app   # a directory
mantis README.md           # one file
```

You'll see two panels: the **file tree** on the left, and the **content** of the
selected file on the right.

## 2. Move around

You don't need to learn anything special — use the arrow keys, or your mouse.

| To do this… | Press… |
| --- | --- |
| Move up / down in the tree | `↑` / `↓` (or `k` / `j`) |
| Open a file / expand a folder | `Enter` or `→` |
| Collapse a folder / go up | `←` |
| Jump between the tree and the file view | `Tab` |
| Scroll the file | `↑` / `↓`, `PageUp` / `PageDown` |
| Quit | `Ctrl+c` (or `q` while the tree is focused) |

> 🖱️ Prefer the mouse? Click a row to select it, click a folder to fold/unfold,
> and use the scroll wheel in whichever panel your cursor is over.

## 3. Find a file fast

Two kinds of search, both fzf-style (just start typing to filter):

- Press `Ctrl+P` to **search by file name**.
- Press `Ctrl+Shift+F` to **search inside files** (full-text).

Use `↑` / `↓` to pick a result and `Enter` to open it. Press `Esc` to close
search. Inside the popup, `Tab` switches between name and content search.

## 4. Peek at git

If you're inside a git repository, you get this for free:

- Tree entries are **colored by git status** (new, modified, deleted).
- Press `Ctrl+b` to toggle **blame** — see who last touched each line.
- Press `H` (while the tree is focused) for the **history** of the current
  file, then pick a revision to view its diff.
- Press `Ctrl+Shift+G` for **git mode**: show *only* changed files, with their diffs.

There's a whole page on this — see [Git Features](git.md).

## 5. When you can't remember a key

You never have to memorize the keymap:

- Press `?` (or `F1`) for **in-app help** with all the keybindings.
- Press `Ctrl+Shift+P` for the **command palette** — type what you want to do (e.g.
  "blame", "theme"), and it shows the action *and* its shortcut.

## 6. Make it yours (optional)

- Press `t` (while the tree is focused) to switch **themes** live (monokai, solarized, catppuccin, and more).
- Want different colors or keybindings permanently? That all lives in a small
  `mantis.toml` file — see [Configuration](configuration.md).

---

That's the whole core experience. From here:

- [Usage & Keybindings](usage.md) — the complete key reference
- [Git Features](git.md) — blame, diffs, and history in depth
- [Configuration](configuration.md) — themes and custom keys
