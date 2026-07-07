# Usage & Keybindings

## Basic usage

```sh
mantis          # view the current directory
mantis path/to/dir  # view a specific directory
mantis file.md      # open a single file directly
```

Press `?` or `F1` for in-app help, and `q` to quit.

## Pager mode

When no `<path>` is given and stdin is piped rather than a terminal, `mantis`
reads stdin instead of walking a directory:

```sh
git diff | mantis          # navigable side-by-side diff
kubectl logs pod | mantis  # highlighted, searchable log output
curl -s https://example.com/file.py | mantis --language python
```

Diff-shaped input (a `diff --git`/`diff --cc` header, an `@@ -` hunk header, or
a `--- `/`+++ ` file-marker pair) renders through the same side-by-side diff
view as git mode, starting in side-by-side layout regardless of the
`[git.diff] side_by_side` setting. Anything else is syntax-highlighted: pass
`--language <name>` (e.g. `--language rust`) to force it, otherwise mantis
sniffs the first line the way `syntect` detects shebangs and mode lines.

The tree pane collapses (there is no path driving the view) and focus starts
in the content pane, but the tree is still there — drag the splitter or
press `Tab` to browse the working directory alongside the piped content.
Keyboard input keeps working normally even though stdin is consumed by the
piped data: mantis reads keys from the controlling terminal (Unix: `/dev/tty`,
Windows: `CONIN$`) instead, the same trick `less` uses. Input is read to EOF
before the UI starts, so very large piped input delays the first frame rather
than streaming incrementally.

## Terminal compatibility

### Keyboard enhancement

Every default binding uses plain `Ctrl+letter`, a bare/Shift letter, or a
named key — combinations that work identically on every terminal and OS.
Ctrl+Shift combinations are deliberately not used: kitty reserves
`ctrl+shift` for its own shortcuts (`kitty_mod`), Windows Terminal binds
`Ctrl+Shift+P`/`Ctrl+Shift+F` itself, and legacy terminals (macOS
Terminal.app, plain xterm, many SSH setups) can't even distinguish them from
plain `Ctrl`. Modifier bindings are matched case-insensitively, so CapsLock
or a stray Shift never breaks a shortcut.

On terminals with the kitty keyboard protocol (CSI-u), mantis additionally
matches bindings by physical key position, so shortcuts work on non-Latin
keyboard layouts.

| Terminal | Keyboard enhancement (layout independence) | Full mouse support |
|---|---|---|
| kitty | ✓ Full | ✓ |
| WezTerm | ✓ Full | ✓ |
| Ghostty | ✓ Full | ✓ |
| Alacritty 0.15+ | ✓ Full | ✓ |
| Windows Terminal | ✓ Full | ✓ |
| iTerm2 | ✓ Partial¹ | ✓ |
| macOS Terminal.app | ✗ | ✓ |
| xterm (plain) | ✗ | Partial² |
| Most SSH clients | ✗ | Depends on client |
| tmux (inside any terminal) | ✗ | ✗ |

¹ iTerm2 supports CSI-u (disambiguation + event types) but may not report
alternate keys for all keyboard layouts.

² xterm supports mouse events but the generic mouse protocol (SGR 1006) lacks
drag and release tracking. Enable `xterm-mouse` in tmux or `XTerm*decTerminalID:
> 280` for full SGR mouse.

On terminals without keyboard enhancement, mantis shows a one-time notice and
notes the limitation in the in-app help (`?` → Getting started).

The best terminals for full mantis keyboard support are **kitty, WezTerm,
Ghostty, Alacritty 0.15+**, and **Windows Terminal**.

## Session persistence

`mantis` automatically remembers your workspace state across restarts:
expanded directories, the last open file, scroll position, and git mode.
State is cached outside the project tree (`~/.local/state/mantis/`
or `%APPDATA%\mantis\`) so it survives re-clones and never writes
dotfiles into the repository. Each workspace root gets its own file under
the `sessions/` subdirectory. To reset the session for a directory, quit
and delete its file from the `sessions/` subdirectory in the state directory.

> 💡 **Can't remember a key?** Press `?` or `F1` for the help overlay, or `Ctrl+P`
> to open the command palette and search for an action by name — it shows you
> the shortcut too. New to `mantis`? Start with the [Quick Start](quickstart.md).

Bindings are editor-style (VS Code / Sublime conventions) and fully remappable
— see [Keybindings](configuration.md#keybindings) for the complete list, the
macOS (`Cmd`) variants, and the `tree:`/`content:` scoping syntax. The tables
below cover the shipped defaults; single letters (`q`, `p`, `t`, …) only work
while the **tree** panel is focused — the content pane's letter keyspace is
kept free, apart from the vim motions below, for future editing features. Any
action not listed with a content-pane key is still reachable from the command
palette (`Ctrl+P`).

## Global

These work no matter which panel is focused.

| Key                    | Action                  |
| ---------------------- | ----------------------- |
| `Ctrl+c`, `q` (tree)   | Quit                    |
| `F1`, `?`              | Toggle help             |
| `Ctrl+P`               | Command palette (fuzzy-find any action) |
| `Tab`                  | Switch panel            |
| `/`                    | Tree filter (tree) / in-file search (content) |
| `Ctrl+T`               | Global fuzzy file-name picker |
| `Ctrl+F`, `f` (tree)   | Content (full-text) search |
| `Ctrl+r`, `F5`, `r` (tree) | Reload tree         |
| `Ctrl+e`, `e` (tree)   | Open current file in `$EDITOR` |
| `y` (tree)             | Copy absolute path to clipboard |
| `Y` (tree)             | Copy path relative to tree root to clipboard |
| `y` (content)          | Copy current line (or selection if any) to clipboard |
| `Y` (content)          | Copy entire file content to clipboard |
| `.` (tree)             | Toggle hidden files     |
| `H` (tree)             | Git history of current file |
| `Ctrl+O`               | Recent files (jump to a recently opened file) |
| `p` (tree)             | Plugin palette (enable/disable plugins) |
| `Ctrl+g`               | Go to line              |
| `Ctrl+b`               | Toggle full-file blame (dedicated pane replacing the tree) |
| `B` (content)          | Toggle single-line blame bar for the active line |
| `t` (tree)             | Theme picker            |
| `Ctrl+D`               | Toggle git mode (changed files + diffs; the pickers above scope to changed files) |
| `F` (tree)             | Toggle flat / tree view in git mode |

## Tree panel

| Key                  | Action                       |
| -------------------- | ---------------------------- |
| `Up`/`k`, `Down`/`j` | Move selection               |
| `Enter`/`Right`/`l`  | Expand directory / open file |
| `Left`/`h`           | Collapse directory / go up   |
| `Backspace`          | Go up one directory (stops at the directory mantis was launched from) |
| `-`/`=`              | Collapse all / expand all    |
| `g`/`Home`, `G`/`End` | Jump to first / last entry  |

## Content panel

The content pane has a **line cursor** (visible as a highlighted full-width row). Use `Up`/`Down` to move it, then press `B` to show a single-line blame bar for the highlighted line.

When full-file blame is toggled on (`Ctrl+b`), the tree panel is replaced by a dedicated blame pane listing every line's commit hash, author, date, and subject, kept in sync with the content cursor. Clicking a row in the blame pane jumps the cursor there and opens the single-line blame bar.

| Key            | Action                       |
| -------------- | ----------------------------- |
| `Up`/`k`, `Down`/`j` | Scroll / move line cursor |
| `PageUp`/`PageDown`  | Page up / down         |
| `Ctrl+Home`/`g`, `Ctrl+End`/`G` | Jump to top / bottom |
| `Left`/`Right` | Horizontal scroll (when wrap off) |
| `Home`/`0`     | Reset horizontal scroll      |
| `Space`        | Toggle fold at cursor        |
| `Ctrl+g`       | Go to line                   |
| `B`            | Blame the active line        |
| `/`            | In-file search               |
| `n`/`N`        | Next / previous hunk (in a diff) |
| `M`            | Toggle raw/rendered markdown (provided by markdown plugin) |

Word wrap, line numbers, JSON pretty-print, side-by-side diff, and the
staged/unstaged diff cycle have no default content-pane key — use the command
palette (`Ctrl+P`) or bind one yourself in `mantis.toml`.

### Rendered plugin content and line numbers

`mantis` has no built-in markdown renderer; install and enable the `markdown` plugin (`p` in-app, or `[plugins.markdown]` in `mantis.toml`) for rendered Markdown. When a plugin renders a file's content, line numbers are hidden in the gutter. This is by design: rendered content collapses blank lines, strips code fences, and restructures formatting, so rendered-line numbers don't correspond to source-file line numbers.

When the markdown plugin is active and rendering a file, you can press `M` (or run "Toggle markdown render (markdown plugin)" from the command palette) to toggle between the raw file content and the rendered view.

## Git features

### Tree colors

Files and folders in the tree are colored by their git status:

| Color  | Meaning |
| ------ | ------- |
| Green  | New / untracked |
| Yellow | Modified |
| Red    | Deleted |
| Gray   | Ignored |

A directory takes the color of the most significant change inside it.

### Status bar

The status bar shows a git summary when inside a repository:

```
[branch  +ahead -behind  N changed]
```

### Git mode and diff navigation

| Key                 | Action |
| ------------------- | ------ |
| `Ctrl+D`             | Toggle git mode — show only changed files; opening a file shows its diff |
| `F` (tree)           | Toggle flat list / nested tree (git mode only) |
| `n` / `N`            | Jump to next / previous change hunk |
| `B` (content)        | Blame the current line: hash, author, date, summary |
| `H` (tree)           | File history — pick a commit to view its diff |

Side-by-side diff and the staged/unstaged diff cycle have no default key —
use the command palette (`Ctrl+P`) or bind one in `mantis.toml`.

## Search popup

Three search entry points cover different needs:

- **`Ctrl+T`** — global fuzzy file-name picker. Opens the same file-name search
  from either panel, regardless of focus. Use this when you want to jump to any
  file in the project by name.
- **`/`** — context-sensitive: in the tree panel it filters file names inline;
  in the content panel (with a file open) it opens the in-file search bar;
  otherwise it falls back to the file-name picker.
- **`Ctrl+F`** (or `f` in the tree panel) — fuzzy content search across
  all files (or changed files in git mode).

Open any search popup and just start typing to filter.
In git mode (`Ctrl+D`), searches are automatically scoped to only the
changed files — the popup title shows "(changed files)" to make this visible.

| Key       | Action                          |
| --------- | ------------------------------- |
| *(type)*  | Filter results                  |
| `Up`/`Down` | Navigate results              |
| `Tab`     | Switch files / content mode     |
| `Enter`   | Open selected result            |
| `Esc`     | Close search                    |
| `Ctrl+A` | Toggle case-sensitive matching (`[Aa]`) |
| `Ctrl+W` | Toggle whole-word matching (`[\b]`) |
| `Ctrl+R` | Toggle regular-expression matching (`[.*]`) |

The toggles apply to content search (`f`) and the in-file search bar (`/`);
the active options are shown as highlighted `[Aa] [\b] [.*]` indicators.

## Command palette

Press `Ctrl+P` to open a searchable list of **every** action, each shown
next to its current keybinding. Type to fuzzy-filter (e.g. "blame", "theme",
"json"), navigate with `Up`/`Down`, and press `Enter` to run the highlighted
command. It's the fastest way to discover what `mantis` can do without
memorizing keys.

## Reporting a bug

Run **"Report a bug (save diagnostics locally)"** from the command palette to
save an anonymous diagnostic report (app version, OS, terminal, workspace
shape — no paths, names, or content) under the state directory, then attach
it to a GitHub issue. See [Telemetry & Bug Reports](telemetry.md) for exactly
what the report contains.

## Git mode history

`H` (while the tree is focused) opens the file's git history in both normal
and git mode. The diff of a selected commit stays on screen and won't be
replaced by live file-watcher updates. Press `Esc` or reload (`Ctrl+r`/`F5`)
to return to the current file (or the working-tree diff in git mode).

## Open in your editor

Press `Ctrl+e` (or `e` while the tree is focused) with a file open to launch
it in your editor. `mantis` uses `$VISUAL`, then `$EDITOR`, falling back to `vim`. The TUI suspends while the editor runs and
resumes when you exit; the file is reloaded afterwards so you see your changes.

> 💡 `$EDITOR` can include arguments — e.g. `export EDITOR="code --wait"` opens
> the file in VS Code and waits for you to close the tab before returning.

## Status bar

The status bar at the bottom of the screen shows context-sensitive information
about the open file:

- **`Ln N`** — the active (highlighted) line number, 1-indexed.
- **`[Language]`** — the detected syntax name from syntect (e.g. `[Rust]`,
  `[Python]`, `[TOML]`). Hidden when the file type is not recognised or when
  viewing a diff.
- **Scroll percentage** — how far through the file the content pane is
  scrolled.
- **Encoding and line endings** — shown when `I` (file info) is toggled on.

## Code folding

Press `Space` to fold or unfold the block at the cursor. A fold gutter appears
in the content pane when foldable regions are detected, and the status bar shows
fold stats. Fold regions come from two sources: a built-in YAML indentation
detector, and language plugins that supply per-file-type regions over the
[plugin protocol](plugins.md). Plugin regions override the built-in output for
their file extension.

Note that folding for a mainstream language like Rust (`.rs`) can be provided by a
language provider plugin — the bundled `rust` plugin registers this way. You must
explicitly enable such plugins in `mantis.toml` (or via the plugin manager popup) for
folding to work on those files.

## JSON pretty-printing

Viewing a JSON file? Use the command palette (`Ctrl+P` → "Toggle JSON
pretty-print") to reformat it with indentation for easier reading, and again
to return to the raw text. Handy for minified `.json`. There's no default key
for this — bind `toggle_pretty_json` in `mantis.toml` if you want one.

## Mouse

- **Click** a tree row to select it — opens a file, or folds/unfolds a
  directory.
- **Double-click** a directory to make it the new tree root.
- **Click** a pane to focus it.
- **Scroll wheel** scrolls whichever pane is under the cursor.
- **Double-click** a breadcrumb segment to navigate to that directory.
- In the search and history popups, **single-click** selects an entry and
  **double-click** activates it.
