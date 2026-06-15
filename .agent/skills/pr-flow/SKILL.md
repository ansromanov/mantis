---
name: pr-flow
description: Open PRs and work from GitHub issues in this repo using the just recipes and gh
---

Use this repo's `just` recipes (which wrap `gh`) for all PR and issue work. Never
hand-roll the branch/push/PR steps — the recipes encode the required rebase-onto-main
discipline.

## Starting work

- **From an issue:** `just issue-start <number>` — creates and checks out a branch
  linked to the issue (via `gh issue develop`) and installs pre-commit hooks. Prefer
  this whenever an issue exists, so the PR auto-links and closes it.
- **Without an issue:** `just new <branch-name>` — branches fresh from `origin/main`.

Always branch from `origin/main`, never from another feature branch.

## Opening the PR

```
just pr-open               # rebase onto origin/main, push --force-with-lease, then gh pr create --fill
just pr-open --draft       # same, passing extra flags through to gh pr create
```

`just pr-open` runs `just pr` first (fetch → rebase → `--force-with-lease`), so a
failing rebase stops the flow loudly for you to resolve conflicts. If the PR closes
an issue, put `Closes #<n>` in the body.

## Inspecting issues

- `just issues` — list open issues (`gh issue list`)
- `just issue <number>` — view one (`gh issue view`)

## Conventions

- Conventional-commit titles (`feat:`, `fix:`, `refactor:`, `docs:`, `chore:`).
- Label issues with the existing set: `bug`, `enhancement`, `performance`, `ux`,
  `refactor`, `documentation`.
- New issues use the templates in `.github/ISSUE_TEMPLATE/`; blank issues are disabled.
