# Action menu (v0.21). Regenerate with: just docs-screenshots
#
# vhs cannot send function keys, so this tape runs against a throwaway project
# whose mantis.toml pins the menu row open and rebinds `menu_bar` to a chord
# vhs can type. What the screenshot shows is the real menu; only the key that
# opened it differs from the documented F10.
Output .vhs-out/menu-bar.png

Require mantis

Set FontFamily "Menlo"
Set FontSize 16
Set Width 1400
Set Height 820
Set Padding 24

Hide
Type "export REPO=$PWD" Enter
Type "export DEMO=${TMPDIR:-/tmp}/mantis-shot-menu" Enter
Type "rm -rf $DEMO && mkdir -p $DEMO/home $DEMO/project" Enter
Type "cp $REPO/docs/tapes/fixtures/deploy-report.json $DEMO/project/" Enter
Type "cp $REPO/docs/tapes/fixtures/deployment.yaml $DEMO/project/" Enter
Type "cp $REPO/docs/tapes/fixtures/quarterly-usage.csv $DEMO/project/" Enter
Type "cp $REPO/docs/tapes/menu-demo/mantis.toml $DEMO/project/" Enter
Type "export HOME=$DEMO/home" Enter
Type "cd $DEMO/project" Enter
Type "clear" Enter
Sleep 500ms
Show

Type "mantis ." Enter
Sleep 4s

# A throwaway HOME means the first-run welcome overlay always appears.
Escape
Sleep 1s

Ctrl+T
Sleep 1s
Type "deploy-report"
Sleep 1500ms
Enter
Sleep 2s

# Open the action menu, then drop the View menu open so the shot shows both
# the menu row and a populated dropdown.
Ctrl+Y
Sleep 1s
Right
Sleep 500ms
Down
Sleep 2s
