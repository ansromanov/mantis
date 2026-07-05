---
name: pr-review
description: Full PR review lifecycle — diff review, auto-fix, PR comment triage, conversation resolution, merge-conflict detection
---

Run the full PR review lifecycle in order. Do not skip phases.

## Invocation

```
/pr-review          # reviews the current branch's open PR
/pr-review <N>      # reviews PR #N (checks it out first)
```

## Phase 0 — Setup

If a PR number is given, check it out:
```bash
just fix <N>
```

Identify the PR and repo:
```bash
PR=$(gh pr view --json number -q .number)
REPO=$(gh repo view --json nameWithOwner -q .nameWithOwner)
```

Abort with a clear message if no open PR exists for the branch.

**Block any in-flight auto-merge before doing anything else.** Repos with an
auto-merge workflow (e.g. `.github/workflows/auto-merge.yml`) can enable
`gh pr merge --auto` mid-review and merge the PR the moment CI + conversation
checks go green — potentially before this review's fixes or comments land.
Cancel any queued/in-progress run tied to the PR's head SHA before starting:
```bash
HEAD_SHA=$(gh pr view "$PR" --json headRefOid -q .headRefOid)
gh api "repos/$REPO/actions/runs?head_sha=$HEAD_SHA" \
  --jq '.workflow_runs[] | select(.status=="in_progress" or .status=="queued") | .id' \
  | xargs -r -I{} gh api -X POST "repos/$REPO/actions/runs/{}/cancel"
```
Confirm the cancel took (`gh api repos/$REPO/actions/runs/<id>/jobs --jq '.jobs[].conclusion'`
should show `cancelled`). Don't re-enable auto-merge yourself when the review
finishes — leave that decision to the human; note in the Phase 6 summary that
it was cancelled and needs a manual `gh pr merge --auto --squash` if still wanted.

## Phase 1 — Merge-conflict check

```bash
git fetch origin main
git merge-base --is-ancestor origin/main HEAD || echo "DIVERGED"
```

`just pr` uses **rebase**, not merge. Do a dry-run rebase to surface conflicts before Phase 5:
```bash
git rebase --no-update-refs origin/main 2>&1 || true
git rebase --abort 2>/dev/null || true
```

If the dry-run rebase reports conflicts:
- List every conflicted file and the nature (both-modified, deleted-by-us, etc.)
- Abort the dry run, then resolve conflicts on the branch directly:
  - Edit each conflicted file (keep incoming changes from the PR, add any new fields from main)
  - `git add <file>` each resolved file
  - `cargo fmt --all && cargo clippy --all-targets -- -D warnings` — must be clean
  - Commit the resolution, then re-run `git rebase origin/main` to completion

If `just pr` in Phase 5 still hits a conflict (new commits landed on main between phases):
- Run `git rebase --abort` if rebase is in progress
- Resolve as above, then retry `just pr`

## Phase 2 — Diff review

Fetch the full PR diff:
```bash
gh pr diff
```

Review for:
- Correctness bugs (logic errors, off-by-one, wrong types, unsafe unwraps in production paths)
- AGENTS.md violations:
  - `unwrap`/`expect` outside tests
  - Alt-modifier keybindings
  - Inline `#[cfg(test)] mod tests { ... }` (must be split → `_test.rs`)
  - Missing `//!` module doc blocks on new `.rs` files
  - New `set_*` plugin actions without `PluginContributions` entry
  - Doc update missing for user-visible feature changes
  - **Module split without test split** — if the PR adds new `.rs` submodules extracted
    from an existing module, the source module's `_test.rs` must be split in the same PR
    so each new submodule has its own `_test.rs`. Flag as `BUG` if missing.
- **Consistency (AGENTS.md → Consistency & performance):**
  - Duplicated logic — a second near-copy of an existing routine (clipboard copy,
    editor/browser launch, overlay key handling, scroll clamp) instead of a shared helper
  - Ad-hoc scroll/cursor math instead of the canonical helpers (`content_scroll_max`,
    single clamp path); input and render disagreeing on bounds
  - Non-uniform overlay behaviour (missing Esc / empty-Backspace / click-outside close)
  - Silently-swallowed user-visible failure (`let _ =` on a config save / clipboard /
    external launch that should surface a status message)
  - Raw `slice[i]` on a derived (non-loop-bounded) index instead of `.get(i)`; selection
    not clamped after a rebuild
- **Performance:**
  - Per-frame `draw_*` doing `O(total)` work/allocation when only the visible window
    renders (bound it to `view_height`); recompute that should be cached by revision/query
  - A reload / watcher tick / plugin re-render that resets scroll/cursor/selection or tears
    down an open overlay on the *same* content (must guard on a genuine content switch)
  - New hot path (per-frame render, large-input parse/search) without a `benches/` case
- **Security:**
  - Untrusted input reaching a shell/process: git/plugin args built from file paths or plugin
    output without escaping; `Command` args from user-controlled data
  - Path traversal — reading/writing paths outside the viewed root from plugin or config input
  - Terminal/ANSI injection: file content, git output, or plugin output written to the screen
    without going through the existing sanitiser
  - Trusting plugin JSON without validating fields/bounds; unbounded reads from a plugin pipe
  - Any hardcoded secret/credential
- Rust style: line length >100, wildcard imports (except `use super::*;` in tests), missing `.clone()` on non-Copy types
- **Test hygiene:** test function name contradicts what the test actually asserts (e.g. name says "X wins" but assertion says Y wins); assertion message contradicts the function name

**Delegate, don't re-implement.** If the `ponytail` plugin is installed, run
`/ponytail-review` for the over-engineering/over-complexity pass and fold its findings in.
If `caveman` is installed, format the per-finding output via `/caveman-review`. When neither
is present, do those passes inline using the rules above. Don't reproduce their logic here.

For each finding output exactly:
```
path/to/file.rs:<line>: [SEVERITY] Problem. Fix.
```
Severity levels: `BUG` (must fix before merge) | `WARN` (should fix) | `STYLE` (optional).

**File every `BUG`/`WARN` finding as a real PR comment before fixing it.** Don't
silently patch and move on — each finding needs an auditable, resolvable thread,
same as a human/Copilot comment gets in Phase 4. Skip filing one only if an
existing unresolved thread (checked via the Phase 4 query) already covers the
same issue.
```bash
HEAD_SHA=$(gh pr view "$PR" --json headRefOid -q .headRefOid)
gh api "repos/$REPO/pulls/$PR/comments" \
  -f body="[SEVERITY] Problem. Fix." \
  -f commit_id="$HEAD_SHA" \
  -f path="path/to/file.rs" \
  -F line=<line> \
  -f side=RIGHT
```
`STYLE` findings are reported inline only (per the rule below) — don't file
comments for those.

## Phase 3 — Auto-fix

Apply all `BUG` and `WARN` findings automatically:
1. Edit files to fix each issue
2. After all edits: `cargo fmt --all`
3. `cargo clippy --all-targets -- -D warnings`
4. `just test-pr`
5. Commit: `git add -p` each changed file, then commit with a message that lists what was fixed

Do NOT auto-fix `STYLE` findings without explicit user approval.

Comments filed in Phase 2 for these findings become new unresolved threads;
Phase 5's `just resolve-threads` resolves them once the fix is committed and
pushed, exactly like pre-existing reviewer threads.

## Phase 4 — Address PR review comments

Fetch all open (unresolved) review threads:
```bash
gh api graphql --paginate \
  -F owner="${REPO%/*}" -F name="${REPO#*/}" -F pr="$PR" \
  -f query='
    query($owner:String!, $name:String!, $pr:Int!, $endCursor:String) {
      repository(owner:$owner, name:$name) {
        pullRequest(number:$pr) {
          reviewThreads(first:100, after:$endCursor) {
            pageInfo { hasNextPage endCursor }
            nodes {
              id
              isResolved
              comments(first:10) {
                nodes { body author { login } path line }
              }
            }
          }
        }
      }
    }' \
  --jq '.data.repository.pullRequest.reviewThreads.nodes[] | select(.isResolved==false)'
```

For each unresolved thread:
1. Read the comment(s) to understand what is requested
2. Apply the fix (edit code, then fmt + clippy as above)
3. Track which threads you addressed

If a comment is ambiguous or contradicts AGENTS.md, note it and skip rather than guessing.

## Phase 5 — Push + resolve threads

```bash
just pr                    # fetch, rebase onto origin/main, push --force-with-lease
just resolve-threads       # mark addressed threads resolved via GraphQL
```

If `just pr` fails mid-rebase with conflicts: resolve them (see Phase 1 resolution steps),
then `git rebase --continue` until done, then `git push --force-with-lease` directly.

## Phase 6 — Summary

Output a structured report:

```
## PR Review Summary — PR #<N>

### Auto-merge
<none in-flight | cancelled run <id>, needs manual re-enable>

### Merge conflicts
<none | list of files + resolution>

### Diff findings applied
<BUG/WARN findings fixed, one line each>

### Diff findings (STYLE — not auto-applied)
<style findings for human review>

### PR comments addressed
<thread summaries, one line each>

### Push
<commit SHA + branch>

### Threads resolved
<count resolved>
```

## Rules

- Never skip `cargo fmt` / `cargo clippy` after editing code.
- Never push without running `just test-pr` first.
- Never resolve a thread you did not address in code.
- File a PR comment for every `BUG`/`WARN` diff finding before fixing it — don't
  silently patch and move on; the fix needs a resolvable, auditable thread.
- Cancel any in-flight auto-merge run in Phase 0 before starting the review; never
  re-enable it yourself afterward.
- Use `isolation: "worktree"` when spawning any subagent during this flow.
- If `just test-pr` fails, fix the failure before pushing — do not push a broken branch.
