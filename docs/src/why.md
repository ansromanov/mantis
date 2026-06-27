# Why mantis?

There are a lot of ways to look at code. So why `mantis`?

Because `mantis` is built for **one job** — moving through a codebase and reading it,
with git context one keystroke away — and it does that job with **zero setup**.
It is deliberately *not* a full editor, and that focus is what keeps it fast and
simple.

> 💡 **The short version:** reach for `mantis` when you want to *explore a repo, read
> a file, or check a diff* without launching a heavyweight editor. When you
> actually need to change something, press `e` to jump into your `$EDITOR`.

## At a glance

| | **mantis** | **Vim / Neovim** | **VS Code** |
| --- | --- | --- | --- |
| Footprint | Single ~MB binary | Light core, heavy once configured | Electron, hundreds of MB + RAM |
| Setup to be useful | **Zero** — just run `mantis` | Hours of config & plugins | Install, extensions, indexing |
| Learning curve | Arrow keys & a mouse | Steep (modes, motions, ecosystem) | Gentle, but mouse-heavy |
| Tree view | **Built in** | Needs a plugin (nvim-tree, etc.) | Built in |
| Fuzzy + full-text search | **Built in** | Needs telescope/fzf/ripgrep glue | Built in |
| Git diff / blame / history | **Built in** | Needs fugitive/gitsigns/etc. | Needs extensions |
| Time to first paint | Milliseconds | Fast (slower with a big config) | Seconds |
| Works great over SSH | **Yes** | Yes | Awkward |

## Compared to Vim / Neovim

Vim and Neovim are superb editors. But to get the everyday browsing experience
`mantis` gives you out of the box — a file tree, a fuzzy finder, full-text search,
git signs, inline blame, and side-by-side diffs — you have to **assemble and
maintain a stack of plugins**:

- a tree plugin (nvim-tree, neo-tree),
- a fuzzy finder (telescope, fzf.vim),
- git integration (vim-fugitive, gitsigns),

…plus a plugin manager and the config glue to hold it together. Curated configs
like **LazyVim** make this easier, but they are large, opinionated systems with
their own learning curve and maintenance.

And then there's the **modal learning curve** itself: modes, motions, registers,
and muscle memory that take real time to build.

`mantis` skips all of that. There's no `init.lua`, no plugin manager, and no modes —
just arrow keys (or `hjkl` if you prefer) and your mouse. Everything listed above
already works the moment you run it.

> 🧭 Love Vim? Keep it. `mantis` is the *browser* you open first; `e` hands the file
> straight to your editor when it's time to write.

## Compared to VS Code

VS Code is a great IDE. But it's an **Electron application** — it bundles a whole
web browser, so it's slow to launch and memory-hungry just to glance at a file or
review a diff. On a remote machine over SSH, that gets even more painful.

`mantis` is a tiny native binary. It opens **instantly**, sips memory, and runs
happily inside any terminal — local or remote. When you only need to *read* code
or *review* a change, you don't need to boot an IDE for it.

## When mantis is the right tool

- ✅ Exploring an unfamiliar repository
- ✅ Reading source, docs, or markdown
- ✅ Reviewing a diff or checking who changed a line (blame)
- ✅ Working on a remote server over SSH
- ✅ Anytime you want something that opens *now*

For heavy, sustained editing — refactors, LSP, debugging — use your editor. `mantis`
gets you there fast with `e`, and is waiting when you come back.
