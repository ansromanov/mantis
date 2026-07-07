# Phase 5 — Push + resolve threads

```bash
just pr                    # fetch, rebase onto origin/main, push --force-with-lease
just resolve-threads       # mark addressed threads resolved via GraphQL
```

If `just pr` fails mid-rebase with conflicts: resolve them (see conflict resolution steps in [merge-conflict.md](file:///home/dbt/projects/mantis/.agent/skills/pr-review/references/merge-conflict.md)), then `git rebase --continue` until done, then `git push --force-with-lease` directly.
