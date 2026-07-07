# Phase 3 — Auto-fix

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
