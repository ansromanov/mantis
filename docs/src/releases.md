# Release Digests

## v0.17: faster navigation, richer review, and broader language support

Version 0.17 makes the command palette the quickest route into a repository,
adds a commit-oriented review flow, and expands the file types that mantis can
read well out of the box. Everything below is available with the default
configuration.

### One palette for commands, files, content, and lines

Press `Ctrl+P` to open the command palette. Its first character chooses what
you are looking for:

| Start the query with | Use it to |
|---|---|
| *(nothing)* or `>` | Find and run a mantis command |
| `/` | Fuzzy-find a file |
| `#` | Search file contents across the repository |
| `:` | Jump to a line; for example, `:120` |

The palette remembers which commands you use. With an empty query, your most
recent command and the most useful recent commands appear first; that ranking
gradually gives more weight to what you use now. Set
`palette_pin_recent = false` or `palette_frequent_count = 0` in `mantis.toml`
if you prefer an unpinned palette.

Plugins can also add actions to this same palette. Plugin authors can use the
`register_commands` protocol action; see [Plugin Development](plugin-development.md#register_commands)
for the message format and lifecycle rules.

### Review a change since any revision

Use **Compare against a revision** from `Ctrl+P` to review the current working
tree against a branch, tag, commit hash, or revspec such as `HEAD~3`. Select a
suggestion or type the revision and press `Enter`. The tree then contains only
files changed since that revision, and selecting a file opens its diff. The
status bar displays the selected comparison base.

There is also a repository-wide commit browser. Press `L` while the tree has
focus, or run **Browse repository commits** from the palette. Type to filter
by hash, author, or subject, select a commit, and press `Enter` to begin a
compare review from that commit. Press `Esc` or `Ctrl+D` to leave compare mode.

See [Git Features](git.md) for the full git workflow.

### Fold structured data and shell scripts

The bundled JSON, YAML, and shell language providers are enabled by default.
Open a `.json`, `.yaml`/`.yml`, `.sh`, `.bash`, or `.zsh` file, focus the
content pane, place the active line on a foldable section, and press `Space` to
collapse or expand it.

- JSON folds multi-line objects and arrays. Use the existing JSON pretty-print
  command first when viewing minified JSON.
- YAML folds indentation-based sections.
- Shell folding recognizes function and compound blocks while ignoring braces
  in comments, quoted strings, and heredocs.

The providers can be disabled individually under `[plugins]` in
`mantis.toml`; [Plugins](plugins.md#bundled-plugins) lists their names.

### More files highlighted without setup

TOML, TypeScript/TSX, and Dockerfile syntax packs now ship enabled. Open a
`.toml`, `.ts`, `.tsx`, `.mts`, `.cts`, or `.jsx` file, or a `Dockerfile` or
`Containerfile`, and mantis selects the matching highlighting automatically.
This includes common configuration files such as `Cargo.toml`, `pyproject.toml`,
and `mantis.toml`.

For every bundled provider and syntax pack, including how to opt out, see
[Plugins](plugins.md#bundled-plugins).

## v0.16: review tools and language-aware reading

Version 0.16 focused on making code review more legible while adding folding
for the languages most commonly encountered in a repository.

### Compare a revision and inspect blame in context

To review all changes since a branch, tag, hash, or revspec, run **Compare
against a revision** from `Ctrl+P`, choose or type the revision, and press
`Enter`. The changed-file tree and each file's diff stay scoped to that base;
press `Ctrl+D` to return to normal browsing.

With a tracked file open, press `Ctrl+b` to open the full-file blame pane.
It follows the content cursor and shows the hash, author, date, and subject for
each line. Press `B` in the content pane when you only need the active line's
blame summary. The [Git Features](git.md) guide covers both workflows.

### Fold Rust, Python, and Go

The bundled Rust, Python, and Go providers add fold regions for `.rs`, `.py`,
and `.go` files. Focus the content pane, move to a foldable declaration or
block, and press `Space` to collapse or reopen it. Folding stays available
alongside the ordinary syntax highlighting and can be disabled per provider in
`mantis.toml` if needed.

### Faster discovery and safer diagnostics

The command palette now presents categories, descriptions, match highlights,
and context when an action cannot apply. Press `Ctrl+P`, type an action name,
and use the displayed shortcut or `Enter` to run it.

Use **Report a bug (save diagnostics locally)** from the palette to create a
local, reviewable report, or **Toggle telemetry** to opt into the local-only
usage log. Neither feature uploads data. See [Telemetry & Bug Reports](telemetry.md)
for the saved locations and contents.

## v0.15: navigation and compatibility polish

Version 0.15 was a focused stability release rather than a new interaction
mode. Large trees avoid an unnecessary deep scan when nothing is expanded, so
starting a repository and moving through its top level is more responsive.

Visual selection now remains visible on the active line, making it easier to
keep your place while copying or inspecting a range. Existing configuration
files using older `git_mode` and renamed keymap fields continue to load without
spurious warnings; consult [Configuration](configuration.md) when you are ready
to migrate to the current names.

## v0.14: pager mode, powerful search, and familiar keys

Version 0.14 made mantis more useful in command pipelines and easier to learn
for users coming from common editors.

### Use mantis as a terminal pager

Pipe a diff, log, or any text into mantis to browse it interactively:

```sh
git diff | mantis
git log -p | mantis
kubectl logs pod | mantis
```

Diff-shaped input opens in a navigable side-by-side view. For other input, use
`--language` when you want to force syntax highlighting. See [Pager mode](usage.md#pager-mode)
for behavior and platform details.

### Tune a search without changing its query

In content search or the in-file search bar, press `Ctrl+A` for
case-sensitive matching, `Ctrl+W` for whole-word matching, and `Ctrl+R` for a
regular expression. The active `[Aa]`, `[\b]`, and `[.*]` indicators show
which constraints are in effect. Press the same shortcut again to turn it off.

### Learn the editor-style defaults

The shipped keymap uses familiar editor-style bindings: `Ctrl+P` opens the
palette, `Ctrl+T` finds files, `Ctrl+F` searches contents, `Ctrl+G` goes to a
line, and `Ctrl+D` opens the changed-file review view. Press `?` or `F1` for
the structured help overlay, then use its tabs to browse key groups. All of
these bindings remain remappable in `mantis.toml`.

Plugin authors should update manifests to `mantis_protocol = "3"` when using
the protocol-3 request/response, key-consumption, or provider-priority
features; [Plugin Development](plugin-development.md#protocol-version) has the
compatibility details.

## v0.13: a more dependable everyday viewer

Version 0.13 improved the tools used to orient yourself in a repository and
made plugin failures easier to diagnose.

### Preview themes and use a scrollable help overlay

Press `t` in the tree to open the theme picker. Moving through its entries
previews each theme immediately; press `Enter` to keep it or `Esc` to return
to the original theme. The [Themes](themes.md) page explains persistent theme
configuration.

Press `?` or `F1` whenever you need a reminder. Help is scrollable, and its Git
section collects the status, diff, blame, and history shortcuts in one place.

### Rely on safer sessions and clearer plugin failures

Each workspace keeps independent session state, so multiple mantis instances
no longer overwrite one another's restored view. Bundled plugins are embedded
with the application rather than relying on a local build at startup, and old
retired shell plugins are removed during upgrade.

If a process plugin exits unexpectedly, mantis keeps its recent stderr and
points to a local plugin log. Open the plugin picker with `p` in the tree to
check its status; [Plugins](plugins.md) explains how to enable, disable, and
configure plugins.
