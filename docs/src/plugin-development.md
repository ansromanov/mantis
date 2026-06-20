# Plugin Development

This page describes the protocol for writing a `tv` plugin. See
[Plugins](plugins.md) for how to install and configure plugins.

## Protocol overview

A plugin is any executable that:

1. Reads newline-delimited JSON objects from **stdin** (events from `tv`).
2. Writes newline-delimited JSON objects to **stdout** (actions back to `tv`).
3. Exits cleanly when it receives `shutdown` (or when stdin closes).

`tv` spawns each plugin as a subprocess with `stdin` and `stdout` piped and
`stderr` discarded (redirected to `/dev/null`). A background reader thread
drains each plugin's stdout and a background writer thread handles stdin so the
`tv` event loop never blocks on plugin I/O.

## Events: tv → plugin (stdin)

Each event is one JSON object on a single line. Unknown fields are ignored.

### `init`

Sent once immediately after spawn, before any user interaction.

```json
{"event":"init"}
```

### `on_file_open`

Sent when the user opens a file in the content panel.

```json
{"event":"on_file_open","path":"/absolute/path/to/file"}
```

### `on_keypress`

Sent on every keypress, including inside overlays. The `key` field uses
human-readable notation: `"q"`, `"ctrl+c"`, `"Enter"`, `"alt+."`.

```json
{"event":"on_keypress","key":"ctrl+p"}
```

### `on_selection_change`

Sent when the tree cursor moves to a different entry. `path` is absent if the
tree is empty.

```json
{"event":"on_selection_change","path":"/absolute/path/to/entry"}
```

### `on_quit`

Sent when the user initiates a quit (before `shutdown`). Use this to do any
final work before the process is torn down.

```json
{"event":"on_quit"}
```

### `shutdown`

Sent as the final event. `tv` closes stdin immediately after sending this.
Exit cleanly in response.

```json
{"event":"shutdown"}
```

## Actions: plugin → tv (stdout)

Respond with action objects on stdout. Each object must be on a single line.
Lines that are not valid JSON or that lack `"event":"action"` are silently
ignored.

### `show_message`

Displays a message in the `tv` status bar.

```json
{"event":"action","action":"show_message","params":{"message":"hello from plugin"}}
```

### `open_file`

Opens a file in the content panel.

```json
{"event":"action","action":"open_file","params":{"path":"/tmp/output.txt"}}
```

## Rules

- **One JSON object per line.** No pretty-printing, no multi-line objects.
- **Stdout is for actions only.** Plugin stderr is discarded; write debug output to a log file instead.
- **Exit on shutdown.** When stdin closes or you receive `shutdown`, exit.
  Do not loop forever waiting for more input.
- **Idempotent reads.** `tv` may send multiple `on_file_open` events for the
  same path if the user reopens a file.
- **No blocking.** Actions are drained non-blockingly every tick. Sending many
  actions in rapid succession is fine; they are buffered and processed in order.

## Minimal Python example

```python
#!/usr/bin/env python3
"""Logs every opened file to ~/.config/tree-viewer/plugins/open-log.txt."""
import json
import sys
from pathlib import Path

LOG = Path.home() / ".config" / "tree-viewer" / "plugins" / "open-log.txt"

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        msg = json.loads(line)
    except json.JSONDecodeError:
        continue

    event = msg.get("event")
    if event == "on_file_open":
        with LOG.open("a") as f:
            f.write(msg.get("path", "") + "\n")
        print(json.dumps({
            "event": "action",
            "action": "show_message",
            "params": {"message": f"logged {msg.get('path','')}"},
        }), flush=True)
    elif event == "shutdown":
        sys.exit(0)
```

## Architecture notes

- `src/plugin.rs` owns the subprocess lifecycle and the background reader
  thread per plugin.
- `PluginManager` collects events into an internal buffer; `App::tick()` drains
  them via `drain_plugin_actions()` in `src/app/refresh.rs`.
- Hook dispatch (`on_file_open`, `on_keypress`, `on_selection_change`) happens
  in `src/app/file_ops.rs`, `src/app/key_handlers/`, and `src/app/navigation.rs`
  respectively.
- Plugin config deserialization lives in `src/config/mod.rs` under the
  `plugins` key.
