---
name: capability-sweep
description: Fan out 3-5 parallel agents that drive the built mantis binary in tmux against real repos, generated corner-case fixtures, and a perf lane, each covering one capability group, then aggregate a pass/fail report.
---

Run a broad end-to-end verification of the mantis TUI by launching several
agents in parallel. Each agent drives the **built release binary** in its own
tmux session against a different target and covers one capability lane. You
(the orchestrator) build once, generate fixtures once, spawn the agents,
then aggregate their reports.

## Invocation

```
/capability-sweep [repo-path ...]
/capability-sweep --quick [repo-path ...]
```

- `repo-path ...` — zero or more paths to extra codebases to test against
  (any language; a mix is good — a docs/markdown repo, a Rust repo, a
  polyglot repo). They are distributed across lanes 1-4 as additional
  targets. If none are given, lanes 1-4 use this repo's own tree plus
  `e2e/data/`.
- `--quick` — smaller fixtures and a shorter scenario list per lane. Use it
  for a fast confidence check; omit it for a real sweep.

Do not hard-code anyone's repo names in this file or in agent prompts — the
caller supplies them as arguments each run.

## Phase 0 — Orchestrator setup (do this yourself, once)

1. **Resolve targets.** Canonicalise each `repo-path` arg; drop any that
   isn't a directory and note it. Record which are git repos
   (`git -C <path> rev-parse --git-dir`).
   **Protect target configs.** mantis persists some in-app settings (theme,
   word wrap, tree width, hidden-files, watch) to the *resolved* config
   path, and a project-local `mantis.toml` outranks `XDG_CONFIG_HOME`. So
   for every target dir (the caller repos, this repo, `e2e/data`'s repo)
   that contains a `mantis.toml`, copy it aside now
   (`cp <target>/mantis.toml <scratch>/save-1.toml`, then increment the
   numeric suffix for each additional target) and restore each copy in Phase
   2. The generated fixture has no `mantis.toml`, so setting-toggle
   scenarios there are safe.
2. **Build the binary.**
   ```bash
   cargo build --release
   ```
   The absolute binary path is `<repo-root>/target/release/mantis`. Every
   agent uses this exact path — agents must never `cargo build` themselves.
3. **Generate the corner-case fixture** into a scratch dir:
   ```bash
   python3 .agent/skills/capability-sweep/gen_fixtures.py <scratch>/capsweep-fixture ${QUICK:+--quick}
   ```
   The last stdout line is the fixture root. It contains `many_files/`,
   `deep/`, `huge.rs`, `wide.txt`, `big.json`, `data.csv`, `sample.png`,
   `binary.bin`, `crlf.txt`, `bom.txt`, `config.yaml`, and an initialised
   git repo with one modified + one untracked file.
4. **Read the scenario catalog.** `e2e/checklists/ghostty.md` is the fullest
   feature/action/expected table in the repo; the platform-specific
   checklists share the same core. Pull each lane's scenarios from there
   plus the lane notes below. `e2e/data/` holds the small language/format
   samples referenced by the checklist.

## Phase 1 — Spawn the lane agents (one message, parallel Agent calls)

Launch **5 agents** (or 3 with `--quick` if the caller asked for fewer):
lanes 1, 2, 4, 5 always; lane 3 only if at least one git-repo target exists
(otherwise fold its must-not-crash checks into lane 1 and say so).

For each agent use `subagent_type: "general-purpose"`. Pass
`isolation: "worktree"` when the host repo allows it; some checkouts (a
committed `.claude` → `.agent` symlink) reject subagent worktrees — then
spawn without isolation. The agents never touch source, so a shared cwd is
fine. Give every agent the **Common agent brief** below verbatim, then its
**lane block**. The prompt must be self-contained: the agent has none of
this context.

### Common agent brief (prepend to every lane prompt)

> You are verifying the mantis terminal UI. Drive the prebuilt binary at
> `<ABS_BINARY_PATH>` — do NOT build, do NOT edit any file, do NOT touch git.
> This is a read-only driving task; report findings only.
>
> **Isolation.** Use a unique tmux session name `capsweep-<lane>-<rand>`.
> Before the first launch, isolate **both** state and config so the run
> reflects mantis's shipped defaults, not whoever's personal `mantis.toml`:
> ```bash
> export MANTIS_STATE_DIR=$(mktemp -d)
> export XDG_CONFIG_HOME=$(mktemp -d)   # no user mantis.toml -> embedded defaults
> touch "$MANTIS_STATE_DIR/welcome_shown.flag"   # welcome overlay won't cover the tree
> ```
> `rm -rf` both dirs at the end. One scenario should still verify the
> welcome overlay by pointing at a second, fresh `MANTIS_STATE_DIR` without
> the flag. If a keybinding in a scenario does nothing, first suspect the
> default binding, not a defect — the default keymap is in
> `src/config/keymap.rs` (`Keymap::default`); check it before reporting FAIL.
>
> **Launch pattern.**
> ```bash
> tmux new-session -d -s <session> -x 140 -y 42
> tmux send-keys -t <session> "MANTIS_STATE_DIR=$MANTIS_STATE_DIR XDG_CONFIG_HOME=$XDG_CONFIG_HOME <ABS_BINARY_PATH> <target-dir>" Enter
> sleep 3
> tmux capture-pane -t <session> -p
> ```
> Send keys with `tmux send-keys -t <session> <keys>`, read the screen with
> `tmux capture-pane -t <session> -p`, quit with `q` (tree focus) or `C-c`.
> Always `tmux kill-session -t <session>` and `rm -rf "$MANTIS_STATE_DIR"
> "$XDG_CONFIG_HOME"` when done, including on failure.
>
> **Nested-tmux limits — do not report these as FAIL.** Inside tmux,
> `Ctrl`+named-key chords (Ctrl+PageUp/Down, Ctrl+Home/End, F-keys) and real
> mouse wheel/click events often do not reach the app. For those:
> - prefer the plain-key or command-palette route (`Ctrl+P` then type the
>   action name — plain `Ctrl`+letter does work);
> - `Ctrl+B` is tmux's own prefix key; `send-keys -t <s> C-b` still delivers
>   it literally, but if it seems dead, drive that action from `Ctrl+P`
>   instead and note it;
> - to exercise a real wheel event, inject the SGR sequence as hex, e.g.
>   wheel-up at col 50 row 10:
>   `tmux send-keys -t <session> -H 1b 5b 3c 36 34 3b 35 30 3b 31 30 4d`
>   (`36 34` = button 64 = wheel-up; `36 35` = 65 = wheel-down);
> - if a scenario genuinely cannot be driven this way, mark it **BLOCKED
>   (tmux)** with a one-line reason — never PASS and never FAIL.
>
> **Don't persist settings into a real repo.** Applying a theme (`t`),
> toggling word wrap, changing tree width (`[` / `]`), toggling hidden files
> (`.`) or the watcher (`W`) all make mantis rewrite its config file — and
> against a real target that file is that repo's own `mantis.toml`. Run any
> such scenario **only against the generated fixture** (it has no
> `mantis.toml`, so the write lands in your throwaway `XDG_CONFIG_HOME`).
> Against a real repo, only *open* those overlays and `Esc` out without
> applying. If you must apply one, note the exact `mantis.toml` you touched
> so the orchestrator can restore it.
>
> **Evidence.** For every scenario capture the relevant `capture-pane`
> lines (trimmed) that prove pass or fail. A crash, panic, garbled frame,
> stuck UI, or non-zero exit is always a FAIL regardless of lane.
>
> **Report format** — end your run with exactly this, nothing after it:
> ```
> ## Lane <N>: <name> — <target>
> | Scenario | Result | Evidence |
> |---|---|---|
> | ... | PASS / FAIL / BLOCKED (tmux) | short quote or note |
>
> FAILs: <count>   BLOCKED: <count>
> Notes: <anything the orchestrator should know — crashes, surprises, env gaps>
> ```

### Lane 1 — core navigation & content

Targets: this repo's tree, `e2e/data/`, and any caller repo-path assigned to
you. Scenarios: tree expand/collapse (`Enter`/`l`/`h`/`-`/`=`), open files by
`Enter`, keyboard nav (`j`/`k`/`g`/`G`/`PageUp`/`PageDown`), `Tab` focus
switch, horizontal scroll (`Left`/`Right`/`0`), `Backspace` root clamp
(expect `Already at root`), quit via `q` and via `Ctrl+C`, syntax
highlighting renders for at least Rust/Python/JSON/Markdown/YAML, terminal
resize (`tmux resize-window`) leaves borders intact. Also: launch once with a
**fresh** state dir and confirm the ` Welcome to mantis! ` overlay appears
and `Esc` dismisses it and it stays gone on relaunch.

### Lane 2 — search & pickers

Targets: this repo's tree + any assigned caller repo-path. Scenarios:
`Ctrl+F` fuzzy file-name search (type a partial name, `Enter` opens it),
full-text search (`f` in tree — type a token that exists, navigate results),
in-file search (`/` with content focused), command palette (`Ctrl+P`: opens,
fuzzy-filters, matching letters bolded, running an action works, Esc closes),
context-aware dimming in `Ctrl+P` outside a git repo (`Blame active line`
shows a dimmed reason), `Ctrl+O` recent files after opening a couple,
go-to-line (`:` in content), inline tree filter, `.` hidden toggle, `t`
theme picker, `?` help overlay tabs.

### Lane 3 — git features  *(only if a git-repo target exists)*

Target: the first git-repo caller path, else the generated fixture's repo.
Scenarios: `Ctrl+D` git mode (tree filters to changed files with status
badges), select a changed file → diff renders, `n`/`N` hunk nav in a diff,
`H` file history overlay + `Enter` on a commit shows its diff, `Ctrl+B` full
blame pane (columns, cursor sync, `j`/`k`, `Esc` closes), `B` single-line
blame bar, `L` repo commit-log overlay, `Ctrl+P` → `Compare against a
revision` → `HEAD~1` → compare badges appear, `Esc` exits compare.

### Lane 4 — format viewers & folding

Targets: `e2e/data/` and the generated fixture root. Scenarios: JSON
auto-prettify + highlight (`json_sample.json` and the fixture's `big.json`),
CSV/TSV table view (`data.csv`), markdown raw/rendered toggle (`M`), code
folding (`Space` on `huge.rs` / a `.py` — needs the bundled language plugin
enabled, check via `p`), YAML anchors/aliases (`config.yaml`), BOM detection
(`bom_utf8_sample.txt` → status shows BOM), CRLF (`crlf_sample.txt` → status
shows CRLF, no visible `\r`), binary block (`binary_sample.bin` /
`binary.bin` → `[binary file — …]`), image metadata (`sample.png` →
`[image file — PNG, 1x1, …]`), long-line handling (`long_lines.txt` /
`wide.txt` with and without word wrap).

### Lane 5 — corner cases & performance

Target: the generated fixture root only (ignore caller repo-paths).
Scenarios:
- **Cold start** — time from launch to first rendered tree
  (`capture-pane` shows `many_files`); should be well under ~2s. Report the
  measured seconds.
- **Many files** — open `many_files/`, `=` expand-all, scroll the tree
  top-to-bottom, run the inline tree filter with a common substring; UI stays
  responsive, no truncated/garbled frames, correct match count.
- **Deep nesting** — navigate into `deep/level_00/.../level_NN`, then
  `Backspace` back out; breadcrumb and tree stay consistent.
- **Huge file** — open `huge.rs`; `G` to bottom, `g` to top, `PageDown`
  spam; no lag spikes, line numbers correct at the end.
- **Wide lines** — open `wide.txt`; toggle word wrap; horizontal scroll.
- **Big JSON** — open `big.json`; prettify completes without hang; scroll.
- **Reload** — `Ctrl+R` on the many-files tree; completes, selection kept.
- **Benchmarks** — run
  `cargo bench --bench performance -- --warm-up-time 1 --measurement-time 3`
  (or `--quick`: add `--sample-size 10`). Capture the `event_parser`,
  `tree_walk`, `content_search`, `file_open`, `highlight` group numbers.
  Flag any that look pathological (e.g. tree_walk > 100ms for 5k files,
  file_open > 50ms). You are reporting numbers + obvious regressions, not
  enforcing a hard baseline unless one is committed under `benches/`.

## Phase 2 — Aggregate (orchestrator)

When all agents return:

1. Print one **summary table**: `Lane | Target | PASS | FAIL | BLOCKED`.
2. List **every FAIL** in full (lane, scenario, evidence) — these are the
   output that matters. Group identical failures across lanes.
3. List **BLOCKED (tmux)** items separately with a note that they need a
   real terminal (or the `run` skill on the host) — they are not defects.
4. Paste lane 5's benchmark numbers verbatim and call out anything it
   flagged.
5. State the environment: binary path/commit, targets used (by role, not by
   anyone's private path), `--quick` or full, fixture counts.
6. Clean up: kill any surviving `capsweep-*` tmux sessions, `rm -rf` the
   fixture dir and any leftover `MANTIS_STATE_DIR` / `XDG_CONFIG_HOME` temp
   dirs the agents reported. **Restore every target `mantis.toml`** from the
   copies saved in Phase 0 (`cp <scratch>/save-1.toml <target>/mantis.toml`,
   using the matching numeric suffix) and confirm each target config is clean
   (`git -C <target> status --porcelain -- mantis.toml` should be empty) —
   report any target whose `mantis.toml` remains dirty.
7. Before believing a keybinding FAIL, cross-check the id against
   `Keymap::default` in `src/config/keymap.rs` — a run that forgot to set
   `XDG_CONFIG_HOME` will have inherited the operator's remaps.

Do not fix anything as part of this skill — it reports. If FAILs warrant
work, hand the list back to the user.

## Notes

- The agents share the one prebuilt binary and never edit source, so tell
  them explicitly not to rebuild; a per-agent worktree (when isolation is
  available) is just a clean cwd.
- Each agent must set `XDG_CONFIG_HOME` to an empty temp dir so mantis uses
  its embedded default keymap. Skipping this makes the operator's personal
  `~/.config/mantis/mantis.toml` remaps look like app bugs (e.g. a default
  `Ctrl+D`/`Ctrl+B` binding "not working" because it was remapped).
- 5 parallel agents each spinning tmux + the TUI is CPU-heavy on a laptop;
  if the machine is small, run with `--quick` or ask the user to reduce to
  3 lanes (1, 4, 5).
- This skill cannot verify true mouse and `Ctrl`+named-key behaviour because
  of the nested-tmux limitation. For those, point the user at the
  `e2e/checklists/*.md` manual passes or the `run` skill executed directly
  on the host terminal.
