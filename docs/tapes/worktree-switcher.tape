# Worktree switcher (v0.18). Regenerate with: just docs-screenshots
#
# Builds a throwaway repository with three linked worktrees so the picker has
# something real to list, without exposing the author's own checkout paths.
Output .vhs-out/worktree-switcher.png

Require mantis

Set FontFamily "Menlo"
Set FontSize 16
Set Width 1400
Set Height 820
Set Padding 24

Hide
Type "export REPO=$PWD" Enter
Type "export DEMO=${TMPDIR:-/tmp}/mantis-shot-worktree" Enter
Type "rm -rf $DEMO && mkdir -p $DEMO/home $DEMO/checkout" Enter
Type "export HOME=$DEMO/home" Enter
Type "cd $DEMO/checkout" Enter
Type "git init -q -b main ." Enter
Type "git config user.email demo@example.com" Enter
Type "git config user.name Demo" Enter
Type "cp $REPO/docs/tapes/fixtures/deploy-report.json ." Enter
Type "cp $REPO/docs/tapes/fixtures/deployment.yaml ." Enter
Type "cp $REPO/docs/tapes/fixtures/quarterly-usage.csv ." Enter
Type "cp $REPO/docs/tapes/demo/mantis.toml ." Enter
Type "git add -A" Enter
Type "git commit -q -m init" Enter
Type "git worktree add -q ../feature-search -b feature/search" Enter
Type "git worktree add -q ../fix-rollout -b fix/rollout" Enter
Type "clear" Enter
Sleep 1s
Show

Type "mantis ." Enter
Sleep 4s

# A throwaway HOME means the first-run welcome overlay always appears.
Escape
Sleep 1s

# Open the worktree switcher from the command palette.
Ctrl+P
Sleep 1s
Type "worktree"
Sleep 1500ms
Enter
Sleep 3s
