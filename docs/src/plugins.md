# Plugins

`tv` supports subprocess-based plugins — standalone executables that hook into
app events and issue actions back to the viewer. Plugins run in separate
processes; `tv` talks to them over stdin/stdout using newline-delimited JSON, so
a plugin can be a shell script, a Python script, a compiled binary — anything
that can read stdin and write stdout.

## Installing a plugin

Plugins live in the default plugin directory:

- **Linux / macOS:** `~/.config/tree-viewer/plugins/`
  (or `$XDG_CONFIG_HOME/tree-viewer/plugins/` if that variable is set)
- **Windows:** `%APPDATA%\tree-viewer\plugins\`

Drop the executable there and make it executable (`chmod +x my-plugin.sh` on
Unix).

## Registering a plugin

Add a `[plugins]` section to your `tv.toml`:

```toml
[plugins]
my-plugin = { path = "my-plugin.sh", enabled = true }
```

- **`path`** — path to the executable. Relative paths are resolved against the
  default plugin directory above. Absolute paths are used as-is.
- **`enabled`** — set to `false` to keep the entry without loading the plugin.

You can register as many plugins as you like:

```toml
[plugins]
git-stats   = { path = "git-stats.sh" }
file-logger = { path = "/usr/local/bin/tv-file-logger" }
dev-tools   = { path = "dev-tools.py", enabled = false }
```

## What plugins can do

Plugins receive lifecycle and hook events from `tv` and can respond with
**actions**:

| Action | Effect |
|---|---|
| `show_message` | Displays a message in the status bar |
| `open_file` | Opens a file in the content panel |

The `show_message` action accepts a `message` parameter; `open_file` accepts a
`path` parameter pointing to an existing file.

## Lifecycle

1. `tv` starts, reads `[plugins]` from config, and spawns each enabled plugin.
2. Each plugin receives an `init` event.
3. As you use `tv`, plugins receive hook events (`on_file_open`,
   `on_keypress`, `on_selection_change`).
4. When you quit, each plugin receives `on_quit` then `shutdown`, and `tv`
   waits for each subprocess to exit.

Plugin stderr is discarded (`/dev/null`). Write debug output to a log file instead.

## Example: status-bar clock

A minimal shell plugin that shows the current time in the status bar on every
file open:

```bash
#!/usr/bin/env bash
# ~/.config/tree-viewer/plugins/clock.sh

while IFS= read -r line; do
    event=$(echo "$line" | python3 -c "import sys,json; print(json.load(sys.stdin).get('event',''))")
    case "$event" in
        on_file_open|init)
            ts=$(date +"%H:%M:%S")
            printf '{"event":"action","action":"show_message","params":{"message":"%s"}}\n' "$ts"
            ;;
        shutdown) exit 0 ;;
    esac
done
```

Register it:

```toml
[plugins]
clock = { path = "clock.sh" }
```

## Bundled plugins

`tv` ships with two optional bundled plugins that extract git diff and file-log
functionality into subprocess-based scripts:

| Plugin | File | What it does |
|---|---|---|
| git-diff | `git-diff.sh` | On `on_file_open`, if the file is git-tracked, shows `git diff --color=always HEAD` as the content (replacing the file view). |
| git-log | `git-log.sh` | On `H` keypress, shows `git log --oneline --color=always` for the current file as a static file view. |

Both are installed to the plugin directory the first time `tv` creates its
global config. The default plugin directory is `~/.config/tree-viewer/plugins/`
on Linux/macOS and `%APPDATA%\tree-viewer\plugins\` on Windows. Enable them by
uncommenting or adding entries in `tv.toml`:

```toml
[plugins]
git-diff = { path = "git-diff.sh" }
git-log  = { path = "git-log.sh" }
```

> **Note:** The bundled plugins provide a simpler implementation than the
> built-in core features:
> - `git-diff` uses git's ANSI colouring rather than tv's theme-aware
>   `diff_line_style` / side-by-side rendering.
> - `git-log` shows the log as a static file; it does not have the interactive
>   commit-selection popup that the built-in `H` key provides.
>
> The core git diff and log code paths remain active as a fallback when the
> plugins are not enabled.
