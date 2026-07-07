---
name: pr-review
description: Full PR review lifecycle — diff review, auto-fix, PR comment triage, conversation resolution, merge-conflict detection
---

Run the PR review lifecycle. Load optional steps and reference materials lazily when needed.

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

## Phase 1 — Merge-conflict check (Optional)

If you need to perform a merge-conflict check or if dry-run rebase is needed, read [merge-conflict.md](file:///home/dbt/projects/mantis/.agent/skills/pr-review/references/merge-conflict.md) for full instructions.

## Phase 2 — Diff review (Common Path)

Fetch the full PR diff:
```bash
gh pr diff
```

Review the diff against the guidelines in [review-checklist.md](file:///home/dbt/projects/mantis/.agent/skills/pr-review/references/review-checklist.md).

**Delegate, don't re-implement.** If the `ponytail` plugin is installed, run
`/ponytail-review` for the over-engineering/over-complexity pass and fold its findings in.
If `caveman` is installed, format the per-finding output via `/caveman-review`. When neither
is present, do those passes inline using the rules in [review-checklist.md](file:///home/dbt/projects/mantis/.agent/skills/pr-review/references/review-checklist.md). Don't reproduce their logic here.

For each finding output exactly:
```
path/to/file.rs:<line>: [SEVERITY] Problem. Fix.
```
Severity levels: `BUG` (must fix before merge) | `WARN` (should fix) | `STYLE` (optional).

**File every `BUG`/`WARN` finding as a real PR comment before fixing it.** Don't
silently patch and move on — each finding needs an auditable, resolvable thread,
same as a human/Copilot comment gets. Skip filing one only if an existing unresolved
thread already covers the same issue.
```bash
HEAD_SHA=$(gh pr view "$PR" --json headRefOid -q .headRefOid)
gh api "repos/$REPO/pulls/$PR/comments" \
  -f body="[SEVERITY] Problem. Fix." \
  -f commit_id="$HEAD_SHA" \
  -f path="path/to/file.rs" \
  -F line=<line> \
  -f side=RIGHT
```
`STYLE` findings are reported inline only — do not file comments for those.

## Phase 3 — Auto-fix (Optional)

If auto-fixing `BUG` or `WARN` findings, read [auto-fix.md](file:///home/dbt/projects/mantis/.agent/skills/pr-review/references/auto-fix.md) for instructions. Do NOT auto-fix `STYLE` findings without explicit user approval.

## Phase 4 — Address PR review comments (Optional)

If there are existing open (unresolved) review threads on the PR, read [address-comments.md](file:///home/dbt/projects/mantis/.agent/skills/pr-review/references/address-comments.md) to retrieve and address them.

## Phase 5 — Push + resolve threads (Optional)

If you made edits, read [push-resolve.md](file:///home/dbt/projects/mantis/.agent/skills/pr-review/references/push-resolve.md) to push your changes and resolve review threads.

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
