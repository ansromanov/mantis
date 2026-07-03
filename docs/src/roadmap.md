# Roadmap

> Last updated: 2026-07-03. Living document — the epics linked below are the
> source of truth for status; this page explains the *why* and the sequencing.

## Positioning: the reading tool for the AI era

mantis's pitch has always been *a fast way to read code without launching an
editor*. The opportunity in front of it: **in agent-driven development, humans
write less and read more.** Every coding-agent session produces diffs someone
must review, logs someone must check, YAML someone must sanity-check. Editors
are optimized for writing; mantis can own the *reading and reviewing* half of
the loop — for humans **and** for the agents themselves.

**Vision:** *mantis is the review cockpit for humans working with AI agents,
and the fastest repo-reading surface for the agents themselves.*

### Personas

1. **Backend engineer** — reads unfamiliar services, reviews PRs and agent
   diffs, jumps between code and JSON/YAML fixtures.
2. **DevOps / SRE** — lives over SSH; reads k8s manifests, Terraform, `.env`s,
   and logs; needs instant startup on remote boxes.
3. **AI engineer / agent operator** — runs coding agents; wants to watch what
   the agent changes in real time, feed context into prompts, and give agents
   tools they can call.

### Non-goals (the moat)

Every feature below stays **read-only and zero-config by default**. mantis will
not grow: editing, LSP/IntelliSense, an embedded terminal, a debugger, or a
plugin-language runtime. When a feature needs to *write*, it hands off (to
`$EDITOR`, to the clipboard, to stdout).

---

## The four pillars

### 1. Agent review cockpit
*Epic: [#469](https://github.com/ansromanov/mantis/issues/469)*

The highest-leverage new surface. The primitives already exist (file watcher,
git status/diff, side-by-side rendering) — this pillar composes them:

- **Prompt-ready copy** — copy selection/file as a fenced markdown block with
  language and `path:line` header; the format every agent prompt wants.
- **Diff-first startup** — `mantis --diff [base]` opens straight into git mode
  against `HEAD`, a branch, or a range (`main...`).
- **Watch mode / review dashboard** — `mantis --review`: a changed-files panel
  that refreshes while the agent edits; hunk-walking across files; per-file
  "seen" marks.
- **Session summary** — on quit, a plain-text list of files/hunks reviewed.

### 2. Machine interface
*Epic: [#470](https://github.com/ansromanov/mantis/issues/470)*

Make mantis useful to the agents themselves. mantis is already a library crate;
expose it without the TUI:

- **Headless JSON CLI** — `mantis cat --range … --json`, `mantis search --json`,
  `mantis tree --json`, `mantis blame --json`.
- **MCP server** — `mantis mcp` exposing file-slice, search, diff, blame, and
  tree tools to any MCP-capable client.
- **Remote-control socket** — `mantis --listen <socket>` so an external agent
  can drive a running viewer ("show the user this diff").
- **Protocol hygiene** — rename the plugin manifest's `tv_protocol` field to
  `mantis_protocol` (with back-compat) before third-party plugins multiply.

### 3. DevOps reader
*Epic: [#471](https://github.com/ansromanov/mantis/issues/471)*

mantis's home turf is the terminal over SSH; teach it operational data, not
just code:

- **Log follow mode** (tail-style) with level/timestamp colorizing and a
  filter bar; **JSONL** rendering for structured logs.
- **JSON path breadcrumb** and a jq-style query bar.
- **CSV/TSV table view** ([#71](https://github.com/ansromanov/mantis/issues/71)).
- **Secret masking** for `.env`/credential-shaped files with a reveal toggle.
- **Remote browsing over SSH**
  ([#359](https://github.com/ansromanov/mantis/issues/359)) — sequenced after
  log mode; it multiplies everything else in this pillar.

### 4. Reading UX polish
*Epic: [#472](https://github.com/ansromanov/mantis/issues/472)*

Continuous ergonomics investment: theme live preview, scrollable help,
onboarding hints, regex/case search toggles, sticky scroll, bookmarks, a
context menu, a jump-back navigation stack, line wrap, search result counts.

### 5. Language intelligence (Rust · Python · Go)
*Epic: [#482](https://github.com/ansromanov/mantis/issues/482)*

Syntax highlighting already covers these; this pillar adds language-*aware*
reading, built in and zero-config like the shipped YAML folding:

- **Code folding** for Rust/Go (brace-based) and Python (indentation-based) —
  starter issue [#483](https://github.com/ansromanov/mantis/issues/483).
- **Symbol outline / go-to-symbol** fuzzy picker (regex-based, deliberately
  not tree-sitter — binary size is a feature).
- **Scope context in the breadcrumb**, which also feeds sticky scroll
  ([#199](https://github.com/ansromanov/mantis/issues/199)).

### Cross-cutting: plugin system hardening

The plugin system is the extension surface for pillars 2, 3, and 5, and a
July 2026 review filed its prerequisites: ship bundled plugins in releases
([#477](https://github.com/ansromanov/mantis/issues/477)), remove the
startup `cargo build` fallback
([#478](https://github.com/ansromanov/mantis/issues/478) — security), plugin
stderr diagnostics ([#479](https://github.com/ansromanov/mantis/issues/479)),
a registry trust model before the install UI ships
([#480](https://github.com/ansromanov/mantis/issues/480)), and protocol v3
(request/response, key consumption, provider priorities —
[#481](https://github.com/ansromanov/mantis/issues/481)).

---

## Sequencing

| Release | Theme | Contents |
|---|---|---|
| **0.13** | Trust & polish | Code-review correctness fixes + quick UX wins — tracking issue [#473](https://github.com/ansromanov/mantis/issues/473); plugin packaging/security fixes [#477](https://github.com/ansromanov/mantis/issues/477)/[#478](https://github.com/ansromanov/mantis/issues/478) |
| **0.14** | DevOps reader | Log follow mode + JSONL, `--diff base` startup, secret masking, CSV tables; code folding for Rust/Go/Python ([#483](https://github.com/ansromanov/mantis/issues/483)) |
| **0.15** | Agent cockpit | `--review` dashboard, remote-control socket, session summary, protocol v3 + rename ([#481](https://github.com/ansromanov/mantis/issues/481)); go-to-symbol picker |
| **0.16** | Machine interface | Headless JSON CLI, then the MCP server on top of it; registry trust model + install UI ([#480](https://github.com/ansromanov/mantis/issues/480)) |
| **1.0** | AI-native viewer | SSH remote ([#359](https://github.com/ansromanov/mantis/issues/359)), keymap refactor ([#298](https://github.com/ansromanov/mantis/issues/298)), comprehensive help ([#304](https://github.com/ansromanov/mantis/issues/304)), plugin registry maturity |

**First two bets:** *prompt-ready copy* (days of work, immediate daily-use hook
for the AI crowd) and *log follow mode* (biggest persona expansion per unit of
effort — the machinery is ~80% built).

## North-star metrics

- Time-to-first-render stays **under 50 ms** — every feature is gated on this.
- Weekly installs (Homebrew / install script).
- Share of sessions using git/diff mode — are we becoming the review tool?
- MCP tool-call volume, once shipped.
