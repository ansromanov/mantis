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
