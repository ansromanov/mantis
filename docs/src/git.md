# Git Features

## Git file history

With a file open in the content panel, press `H` to open an fzf-style list of
the commits that touched it. Type to fuzzy-filter, navigate with `↑`/`↓`, and
press `Enter` (or double-click) to load the diff of that revision against your
current working tree into the content panel — additions in green, deletions in
red.

Requires `git` on your `PATH` and the file to be tracked in a repository.

## Git mode

Press `Ctrl+G` to switch the tree to show only files with uncommitted changes
(modified, new, deleted, or renamed). Selecting a file shows its working-tree
diff in the content panel instead of the file contents. The tree title displays
a `[git]` badge while active.

Press `Alt+G` inside git mode to toggle between the tree view (directories
intact) and a flat, depth-0 list of every changed file with relative paths.
Press `Alt+G` again to return to the tree view (a no-op outside git mode).

All directories containing changes are auto-expanded when entering git mode.
Diffs refresh on the 30-second auto-reload tick and on manual `r`.

Configure via `tv.toml`:

```toml
git_mode = false         # start in git mode (default: false)
git_mode_flat = false    # start in flat list view (default: false)
git_status = true        # colour tree entries by git status (default: true)
git_show_deleted = false # show ghost nodes for deleted tracked files (default: false)
```
