# Agent isolation

Always pass `isolation: "worktree"` when spawning subagents via the Agent tool. This ensures each agent works in its own git worktree so parallel edits never conflict.
