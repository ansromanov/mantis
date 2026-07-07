# Phase 1 — Merge-conflict check

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
