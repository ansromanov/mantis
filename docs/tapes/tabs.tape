# Multiple project tabs (v0.20). Regenerate with: just docs-screenshots
#
# Two small throwaway projects, so the tab labels read as project names rather
# than as whatever the author's checkout happens to be called.
Output .vhs-out/tabs.png

Require mantis

Set FontFamily "Menlo"
Set FontSize 16
Set Width 1400
Set Height 820
Set Padding 24

Hide
Type "export REPO=$PWD" Enter
Type "export DEMO=${TMPDIR:-/tmp}/mantis-shot-tabs" Enter
Type "rm -rf $DEMO && mkdir -p $DEMO/home $DEMO/api $DEMO/web" Enter
Type "cp $REPO/docs/tapes/fixtures/deploy-report.json $DEMO/api/" Enter
Type "cp $REPO/docs/tapes/fixtures/deployment.yaml $DEMO/api/" Enter
Type "cp $REPO/docs/tapes/demo/mantis.toml $DEMO/api/" Enter
Type "cp $REPO/docs/tapes/fixtures/quarterly-usage.csv $DEMO/web/" Enter
Type "cp $REPO/docs/tapes/demo/mantis.toml $DEMO/web/" Enter
Type "export HOME=$DEMO/home" Enter
Type "cd $DEMO" Enter
Type "clear" Enter
Sleep 500ms
Show

# Two roots on the command line open as two tabs.
Type "mantis api web" Enter
Sleep 4s

# A throwaway HOME means the first-run welcome overlay always appears.
Escape
Sleep 1s

Ctrl+T
Sleep 1s
Type "deploy-report"
Sleep 1500ms
Enter
Sleep 3s
