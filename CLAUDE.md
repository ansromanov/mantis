# Agent isolation

Always pass `isolation: "worktree"` when spawning subagents via the Agent tool. This ensures each agent works in its own git worktree so parallel edits never conflict.

# Agent workflow

When starting work on a new feature or fix, create a branch with:

```bash
just new your-branch-name
```

This fetches latest main, creates the branch from `origin/main`, and installs pre-commit hooks.

Before pushing and opening a PR, run:

```bash
just pr
```

This fetches latest `origin/main`, rebases onto it (fails loudly on conflicts), and pushes with `--force-with-lease`. Then use `gh pr create` to open the PR.
