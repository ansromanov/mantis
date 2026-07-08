# End-to-End (E2E) & Functional Testing

To ensure that releases do not introduce regressions (especially regarding terminal rendering, PTY initialization, and UI interactivity across different terminals), `mantis` uses a combination of automated and manual E2E/functional testing.

---

## 1. Test Dataset (`e2e/data/`)

A dedicated test dataset is located in the `e2e/data/` directory. It contains static sample files representing different edge cases and supported file types:

- **[rust_sample.rs](../../e2e/data/rust_sample.rs)**: Rust code structure to verify syntax highlighting, search, and line numbers.
- **[json_sample.json](../../e2e/data/json_sample.json)**: Minified JSON to verify automatic pretty-printing.
- **[yaml_sample.yml](../../e2e/data/yaml_sample.yml)**: Nested YAML with anchors (`&`) and aliases (`*`) to verify indentation-based folding and counts.
- **[python_sample.py](../../e2e/data/python_sample.py)**: Python source file.
- **[markdown_sample.md](../../e2e/data/markdown_sample.md)**: Markdown document.
- **[long_lines.txt](../../e2e/data/long_lines.txt)**: Extremely long lines of text to verify word-wrapping behavior.
- **[crlf_sample.txt](../../e2e/data/crlf_sample.txt)**: Text file with Windows (`\r\n`) line endings to verify normalization to LF.
- **[bom_utf8_sample.txt](../../e2e/data/bom_utf8_sample.txt)**: UTF-8 file with a Byte Order Mark (BOM) to verify BOM detection.
- **[binary_sample.bin](../../e2e/data/binary_sample.bin)**: Binary data containing NUL bytes to verify binary placeholder rendering.

---

## 2. Automated E2E Testing

Automated E2E tests are split into two parts:

### A. Cargo Integration Tests (`tests/e2e_tests.rs`)
Runs programmatically using a simulated `App` state and a `TestBackend` to verify file parsing, encoding detection, search matching, and folding logic.

### B. Whole-Binary TUI Smoke Test (`scripts/ci-e2e.py`)
Spawns the compiled `mantis` binary under a pseudo-terminal (PTY) in Python, sets the terminal size, waits for it to render the TUI screen, verifies file tree listings, and exits cleanly with a simulated `q` keystroke. This verifies real-world raw-mode terminal initialization.

### Running Automated E2E Tests
To run all automated E2E tests locally:
```sh
just test-e2e
```
These tests are also run automatically in CI on every Pull Request (in `ci.yml`) and push to main (in `main.yml`).

---

## 3. Manual Testing Checklist

Run through these verification steps on your target terminals before a release:
* **Windows Terminal** (PowerShell & WSL)
* **iTerm2**
* **Ghostty**
* **Alacritty**

### Launch Command
Launch `mantis` targeting the test dataset directory:
```sh
cargo run -- ./e2e/data
```

### Checklist Steps

| Category | Steps to Execute | Expected Behavior |
|---|---|---|
| **1. File Tree & Nav** | Navigate tree with Up/Down arrows or mouse scroll wheel. Double-click or press Enter on directories. | Smooth movement without cursor drift or overlaps. |
| **2. Binary Files** | Select `binary_sample.bin`. | Displays the binary placeholder: `[binary file — BIN file, 125 B]` and shows instructions. |
| **3. JSON Pretty Print** | Select `json_sample.json`. | The minified JSON is pretty-printed across multiple lines, highlighted, and supports folding. |
| **4. YAML Folding** | Select `yaml_sample.yml`. Focus content pane (Tab). Move cursor to a parent line (e.g. `production:`) and press `Space`. | The block collapses. Gutter shows folding indicators (`+` / `-`). Scrolling is adjusted correctly. |
| **5. Word Wrap** | Select `long_lines.txt`. Toggle word wrap via the command palette (`Ctrl+P` and type `wrap`) or by pressing its keybinding (if configured). | Long lines wrap cleanly at terminal edge, line numbers align to physical lines, no horizontal scroll needed. |
| **6. Search** | Select `rust_sample.rs`. Press `/` to open search. Type `Rectangle`. Press Enter. Press `n`/`N` to cycle. | Selection highlights and cursor jumps to each matching keyword. |
| **7. Status Bar** | Check status bar while cycling files. | Correct file names, encoding (e.g., `UTF-8 BOM` for `bom_utf8_sample.txt`), line endings (`CRLF` for `crlf_sample.txt`), and syntax names. |
| **8. Git Diffs** | Press `Ctrl+D` to enter git mode (if in git repository) or view diff history by pressing `H` (from the tree). Toggle side-by-side diff via the command palette. | Displays diff correctly. Side-by-side mode splits left/right panels. |
| **9. Resizing** | Resize the terminal window while running `mantis`. | Viewport adapts immediately without crashing or breaking layout boundary lines. |
| **10. Root Clamp** | With the tree focused at the launch directory, press `Backspace`. | No-op; status bar shows `Already at root`. |
| **11. Compare Mode** | In a git repo, run `Compare against a revision` from the command palette (`Ctrl+P`), enter a revision at the `rev: ` prompt. | Tree shows `A`/`M`/`D`/`R` badges; status bar shows a `[compare: <rev>]` badge. |
| **12. Blame Rework** | Focus content pane on a tracked file, press `Ctrl+B` for the full blame pane, `B` for the single-line bottom bar. | Blame pane shows hash/author/date/subject columns; bottom bar shows a 2-line summary. |
| **13. Bundled Language Plugins** | Press `p` to open the plugin manager, enable `rust`/`python`/`go` (disabled by default). | Folding (`Space`) works on the corresponding language files. |
| **14. Bug Report & Telemetry** | Run `Report a bug (save diagnostics locally)` and `Toggle telemetry` from the command palette. | Bug report modal opens with a scrollable diagnostic preview; telemetry toggle shows `telemetry enabled`/`disabled` in the status bar. |
| **15. First-Run Welcome Overlay** | Launch `mantis` against a fresh state dir (`MANTIS_STATE_DIR=$(mktemp -d) cargo run -- ./e2e/data`). | A dismissible welcome popup shows 5 essential keybindings; does not reappear after `Esc` and relaunch. |

See the per-terminal checklists in `e2e/checklists/*.md` (section "4. New Features") for detailed step-by-step coverage of items 10–15.
