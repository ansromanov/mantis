# CLAUDE.md

Project conventions — architecture, code style, testing layout, and the branch/PR
workflow — live in **[AGENTS.md](AGENTS.md)**. It is the single source of truth and is
read by both Claude Code and opencode. Read it first.

This file holds only the Claude-Code-specific guidance that AGENTS.md does not cover.

## Subagents

Always pass `isolation: "worktree"` when spawning subagents via the Agent tool. Each
agent then works in its own git worktree, so parallel edits never conflict.
