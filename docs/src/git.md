# Git Features

`mantis` has comprehensive git support built in. Inside any git repository you get
status colors, blame, file history, and a dedicated diff-review mode out of the
box.

All git data — repo info for the status bar, per-path status for tree coloring,
blame annotations, and file diffs — is provided natively via the `git` CLI.

> ℹ️ All git features need `git` on your `PATH` and the file to be tracked in a
> repository.

## Git status colors

Whenever `mantis` can read `git status`, tree entries are tinted by their state — new,
modified, deleted, or ignored — so you can see at a glance what's changed without
doing anything. Control this with `git_status` in your config (see
[Configuration](configuration.md)).

The status bar also shows how far the current branch is from its upstream when a
tracking branch exists, using the familiar `↑3 ↓1` summary.

## Git blame

Press `Ctrl+b` with a file open to toggle **full-file blame**. The tree panel
is replaced by a dedicated blame pane listing every line's short commit hash,
author, relative date, and subject. Navigate it with the usual tree
keys (`↑`/`↓`, `PageUp`/`PageDown`, `g`/`G`) — they move the cursor through
the file instead of the tree selection while the pane is open. Press `Ctrl+b`
again (or `Esc`) to close it and return to the tree.

Press `B` with the content panel focused to toggle a **single-line blame
bar** at the bottom of the tree panel, showing the commit hash, author, date,
and subject for the current cursor line.

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

## Repository commit log

Press `L` while the tree is focused, or use the command palette (`Ctrl+P`) to
run **Browse repository commits**, to search the repository-wide commit log.
The picker shows each commit's short hash, date, author, and subject. Type to
filter by hash, author, or subject; select a commit and press `Enter` to review
all changes since it in compare mode. Press `Esc` to close the picker.

## Git mode

Press `Ctrl+D` to switch the tree to show **only files with uncommitted changes**
(modified, new, deleted, or renamed). Selecting a file shows its working-tree
diff in the content panel instead of the file contents. The tree title displays a
`[git]` badge while active — perfect for reviewing everything you're about to
commit.

Press `F` (while the tree is focused) inside git mode to toggle between the
tree view (directories intact) and a flat, depth-0 list of every changed file
with relative paths. Press `F` again to return to the tree view (a no-op
outside git mode).

When the working tree is clean (no uncommitted changes), the tree panel shows a
"Working tree clean" placeholder instead of an empty list, so you can tell at a
glance that there is simply nothing to review. If the current directory is not a
git repository, the placeholder says "Not a git repository" instead. Press
`Ctrl+D` to exit git mode in either case.

All directories containing changes are auto-expanded when entering git mode.
Diffs refresh on the 30-second auto-reload tick and on manual `r`.

`git_status` controls whether tree entries are coloured by git status at startup:

```toml
git_status = true        # colour tree entries by git status (default: true)
git_show_deleted = false # show ghost nodes for deleted tracked files (default: false)
```

## Compare mode

To review changes against something other than the working tree's usual
baseline, open the command palette and run **Compare against a revision**.
A picker overlay opens listing shortcuts (`HEAD`, `HEAD~1`, `HEAD~2`),
local branches, tags, and recent commits. Start typing to fuzzy-filter the
list, or enter any revision (a commit hash, tag, branch name, or something
like `HEAD~3`) freely — press `Enter` to select the highlighted item, or
when no items match, the typed text is used as a raw revspec. `mantis`
switches into git mode: the tree shows only files changed between that
revision and your working tree, and opening a file shows
`git diff <rev> -- <file>` instead of the usual working-tree diff. The
status bar shows a `[compare: <rev>]` badge while active.

Press `Ctrl+D` to leave compare mode and return to full browsing.

## Using mantis as `git`'s pager

`mantis` can read a diff from stdin (see [Pager mode](usage.md#pager-mode)),
so it works as a drop-in side-by-side pager for `git diff`, `git show`, and
`git log -p`:

```sh
git diff | mantis                      # one-off
GIT_PAGER=mantis git log -p            # one-off, any pager-using command
git config --global core.pager mantis  # every git command, permanently
```
