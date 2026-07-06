# Mantis Markdown Sample

This is a sample markdown file used for end-to-end (E2E) testing.

## Features Checklist

- [x] Syntax Highlighting
- [x] Folding Support
- [ ] LSP (Not planned)

### 1. Code Block (Rust)

Here is some code in Markdown:

```rust
fn hello_world() {
    println!("Hello from Markdown code block!");
}
```

> [!NOTE]
> This is a callout in Markdown, similar to GitHub alerts.

### 2. Navigation Details

You can navigate around the directory using standard keybindings:
* `j` / `k` or arrows to move up/down.
* `Enter` or `Right` to expand folders.
* `Left` to collapse folders.

```bash
# To run mantis:
mantis /path/to/repo
```

### 3. Git Integration

When running in git mode, you can inspect:
- Working tree changes.
- Staged commits.
- Git blame annotations.

> [!TIP]
> Press `Ctrl+D` to toggle git mode instantly!

### 4. Configuration

Mantis uses a TOML file for settings:

```toml
[tree]
show_hidden = false
width = 30

[search]
context_lines = 3
```

### 5. Plugin Architecture

Plugins run in subprocesses communicating via JSON lines:

```json
{"event": "file_opened", "path": "/src/main.rs"}
```

This architecture isolates plugins from the main process.
