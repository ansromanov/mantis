# Git Features

One of the best reasons to use `tv` is that **git is built in** — no plugins, no
extensions. Inside any git repository you get status colors, blame, file history,
and a dedicated diff-review mode out of the box.

> ℹ️ All git features need `git` on your `PATH` and the file to be tracked in a
> repository.

## Git status colors

Whenever `tv` can read `git status`, tree entries are tinted by their state — new,
modified, deleted, or ignored — so you can see at a glance what's changed without
doing anything. Control this with `git_status` in your config (see
[Configuration](configuration.md)).

The status bar also shows how far the current branch is from its upstream when a
tracking branch exists, using the familiar `↑3 ↓1` summary.

## Git blame

Press `b` with a file open to toggle **inline blame**. A gutter appears to the
left of the content showing, for each line, the short commit hash, the author,
and the date it was last changed. Press `b` again to hide it.

Blame is disabled while you're viewing a diff (it only annotates real file
content).

### Visual-line blame

For inspecting a specific range rather than the whole file, press `V` with a file
open to enter **visual-line mode**. The first visible line is selected; extend the
selection a line at a time with `j`/`k` (or `↑`/`↓`), jump to the top/bottom of
the file with `g`/`G`, and page with `PageUp`/`PageDown`. The selected lines are
highlighted with a distinct background.

Press `b` while in visual-line mode to open a **blame panel** scoped to the
selection: each line in the range is listed with its short commit hash, author,
relative date, and content. Press `b` again to dismiss the panel, and `Esc` to
leave visual-line mode entirely. Like the inline gutter, this is unavailable
while viewing a diff.

## Git file history

With a file open in the content panel, press `H` to open an fzf-style list of the
commits that touched it. Type to fuzzy-filter, navigate with `↑`/`↓`, and press
`Enter` (or double-click) to load the diff of that revision against your current
working tree into the content panel — additions in green, deletions in red.

## Git mode

Press `Ctrl+G` to switch the tree to show **only files with uncommitted changes**
(modified, new, deleted, or renamed). Selecting a file shows its working-tree
diff in the content panel instead of the file contents. The tree title displays a
`[git]` badge while active — perfect for reviewing everything you're about to
commit.

Press `Alt+G` inside git mode to toggle between the tree view (directories
intact) and a flat, depth-0 list of every changed file with relative paths. Press
`Alt+G` again to return to the tree view (a no-op outside git mode).

All directories containing changes are auto-expanded when entering git mode.
Diffs refresh on the 30-second auto-reload tick and on manual `r`.

You can have `tv` start directly in git mode via `tv.toml`:

```toml
git_mode = false         # start in git mode (default: false)
git_mode_flat = false    # start in flat list view (default: false)
git_status = true        # colour tree entries by git status (default: true)
git_show_deleted = false # show ghost nodes for deleted tracked files (default: false)
```
