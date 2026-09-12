# Release Digests

What each release changed, in the order you would meet it. Every feature below
works with the default configuration unless the text says otherwise.

## v0.21: menus, a clickable status bar, and tabs that scale

*Released 11 September 2026.*

Version 0.21 is about pointing at things. Everything mantis could already do
from the keyboard became reachable by mouse, and the tab strip grew up.

### Find any action without knowing its key

Press `F10` for the action menu: every action grouped by what it does, each
showing the key that runs it. Actions that cannot apply right now — a git
command outside a repository, a fold command in a file with no fold regions —
appear dimmed rather than missing, so the menu doubles as an explanation of
why something is unavailable.

Set `[ui] menu_bar = true` in `mantis.toml` to keep the menu row permanently
visible; hover a menu name to open it and click an action to run it. `F10`
works either way.

<figure>
  <img src="media/docs/menu-bar.png"
       alt="The mantis action menu open over a source file, showing grouped actions with their keybindings.">
  <figcaption>The action menu, opened with F10 — actions grouped by category, each with its current keybinding.</figcaption>
</figure>

### Click the status bar

The status bar became an instrument panel rather than a readout. Click the
git, worktree, line, language, fold, error, update, or file-info segment to
open its picker or run its action — the branch segment opens the revision
picker, the worktree count opens the worktree switcher, the line number opens
go-to-line. Segments hidden because the terminal is narrow are not clickable,
since they are not there to click.

### Tabs at any count

Tab handling now survives having a lot of them:

| Key | Action |
|---|---|
| `Ctrl+1` … `Ctrl+9` | Select tab 1–9 |
| `Ctrl+0` | Select the last tab |
| `Ctrl+[` / `Ctrl+]` | Move the current tab left / right |
| `Ctrl+Tab` | Search open tabs |
| `Ctrl+Backspace` | Reopen the last closed tab |

When tabs outgrow the strip, labels shrink toward a minimum and the strip
scrolls to keep the active tab visible. Click the `‹`/`›` edge affordances or
use the mouse wheel over the strip to reach the rest.

### Overlays that close when you expect

A batch of interaction fixes: the theme picker closes on a single `Esc`,
compare mode exits reliably, and search navigation keys work again while a
query is being typed. See [Usage & Keybindings](usage.md) for the full map.

## v0.20: one mantis, several projects

*Released 8 September 2026.*

Version 0.20 made mantis multi-project. Until now one process meant one
repository; now it means as many as you want, each with its own tree, content
pane, and remembered position.

### Open more than one project

Pass several paths at once (`mantis dir1 dir2`), or press `Ctrl+n` and type a
directory to open it as a new tab. A tab strip appears above the panes as soon
as a second project is open — click a tab to switch, or its `×` to close.

Each tab is a full, independent mantis: its own expanded directories, open
file, scroll position, and `mantis.toml`. The set of open projects is restored
when you relaunch, so a morning's layout survives a reboot.

### Folding for six more formats

Bundled language providers now fold TypeScript and TSX, Go templates, CSS,
TOML, INI, and SQL. Open a file, put the cursor on a foldable construct, and
press `Space`. Every provider can be disabled individually under `[plugins]`
in `mantis.toml`; [Plugins](plugins.md#bundled-plugins) lists them.

### A worktree switcher that opens instantly

The worktree picker no longer waits for git before drawing. It appears
immediately with every worktree listed, and changed-file counts fill in as the
background scan finishes.

## v0.19: structured data, inline images, and a safer plugin registry

*Released 6 September 2026.*

Version 0.19 went after the files that are technically text but miserable to
read as text — big JSON documents, Kubernetes manifests — and taught mantis to
show pictures.

### Navigate JSON by path

Open a JSON file and the status bar shows the path to the value under the
cursor, so you always know where you are inside a deep document. To narrow the
view, run **Open JSON query bar** from `Ctrl+P` and filter or project with a
jq-flavoured subset: `.a.b[0]`, `.items[]`, `{name, image}`. It works on JSONL
as well.

<figure>
  <img src="media/docs/json-query.png"
       alt="A deployment report JSON file in mantis with the jq query bar open at the bottom of the content pane.">
  <figcaption>The query bar, open over a deployment report — jq-flavoured filters and projections without leaving the viewer.</figcaption>
</figure>

### Know what a manifest is before you read it

A bundled `k8s` plugin recognises Kubernetes manifests and summarises them in
the status bar: the first resource's identity and per-kind counts across
`---`-separated documents, for example
`Deployment/nginx (default) · 3 Deployments · 2 Services · 1 ConfigMap`. Files
without both `apiVersion` and `kind` are left alone.

It rides on a new plugin capability, `status_facts`, which any language
provider can use to contribute a short status-bar summary — see
[Plugin Development](plugin-development.md).

<figure>
  <img src="media/docs/k8s-facts.png"
       alt="A Kubernetes deployment manifest in mantis, with the resource identity reported in the status bar.">
  <figcaption>The k8s plugin identifies the manifest — <code>Deployment/nginx (default)</code> — in the status bar, next to the fold count.</figcaption>
</figure>

### See images without leaving the terminal

On terminals that speak the Kitty graphics protocol — Ghostty, Kitty, WezTerm
— images render inline in the content pane. Set `content.image_preview = false`
to turn it off. Everywhere else, the binary-file placeholder now reports the
image's real dimensions instead of only its size.

### Pin the files you keep coming back to

Press `B` to bookmark the open file and `b` to open the bookmarks picker.
Bookmarks are per workspace and restored across sessions, so each project keeps
its own shortlist.

### Terraform folding and a stricter registry

HCL joins the folding set: `.tf`, `.tfvars`, and `.hcl` files fold blocks
correctly through comments, quoted strings, and heredocs.

The plugin registry gained a trust model. Installing a plugin now requires a
`sha256` digest in the registry index, and mantis refuses an artifact whose
digest is missing or does not match. [Plugin Registry](plugin-registry.md)
documents the format.

## v0.18: logs, tables, worktrees, and a right mouse button

*Released 28 August 2026.*

Version 0.18 pushed mantis past source code into the other files a working day
is made of — logs, exports, credentials, manifests — and made the mouse a
first-class way to drive it.

### Follow a log while it is being written

Press `F` to follow a file: mantis stays pinned to the tail as new lines
arrive. Scroll up to read back and follow quietly unpins; return to the bottom
with `G` or `End` and it re-pins. Press `&` to open the filter bar and reduce
the view to lines containing a query.

### Read CSV and TSV as tables

`.csv` and `.tsv` files render as aligned tables with headers and box-drawing
borders, quotes and escapes handled per RFC 4180. Wide tables scroll sideways
with `Left`/`Right`; **Toggle CSV/TSV table view** in the palette returns to
raw text. Files past `prettify_size_limit` stay raw, as with JSON.

<figure>
  <img src="media/docs/csv-table.png"
       alt="A CSV file rendered in mantis as an aligned table with headers and box-drawing borders.">
  <figcaption>CSV and TSV files render as aligned tables, with the raw text one palette action away.</figcaption>
</figure>

JSONL files render one row per record, each expandable in place, so a
structured log reads as a list instead of a wall.

### Credentials stay masked unless you ask

Credential-shaped values in `.env` and similar files are masked by default,
with a fixed-width placeholder so the length of a secret is not exposed
either. **Toggle secret reveal** in the palette unmasks the current file for
the session only — nothing is persisted. Set `content.mask_secrets = false` to
disable the behaviour entirely.

### Move between worktrees

**Open worktree switcher** lists every linked worktree of the current
repository with its branch and changed-file count, and `Enter` re-roots mantis
there. Each worktree keeps its own session state, so switching back returns you
to where you were. When a repository has more than one worktree, the status bar
says so.

<figure>
  <img src="media/docs/worktree-switcher.png"
       alt="The mantis worktree switcher listing linked worktrees with their branches and changed-file counts.">
  <figcaption>Every linked worktree with its branch and change volume; Enter re-roots mantis there.</figcaption>
</figure>

### Right-click anywhere useful

Right-click a tree row or the content pane for a context menu at the cursor:
open, open in editor or the default application, reveal in the file manager,
copy absolute or relative path, expand or collapse, and the word-wrap and
raw-markdown toggles. Navigate with the mouse or `j`/`k`, activate with `Enter`
or a click, dismiss with `Esc`.

### Smaller things that add up

`nginx.conf` and `Justfile` get syntax highlighting. Narrow terminals explain
themselves instead of rendering a broken frame. Blame mode and git mode stay
visible in the status bar when the content pane has focus. Piped input
(`git diff | mantis`) is fully interactive.

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
