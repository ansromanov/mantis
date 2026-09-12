# JSON path breadcrumb (v0.19). Regenerate with: just docs-screenshots
Output .vhs-out/json-query.png

Require mantis

Set FontFamily "Menlo"
Set FontSize 16
Set Width 1400
Set Height 820
Set Padding 24

Hide
Type "export REPO=$PWD" Enter
Type "export DEMO=${TMPDIR:-/tmp}/mantis-shot-json" Enter
Type "rm -rf $DEMO && mkdir -p $DEMO/home $DEMO/project" Enter
Type "cp $REPO/docs/tapes/fixtures/deploy-report.json $DEMO/project/" Enter
Type "cp $REPO/docs/tapes/fixtures/quarterly-usage.csv $DEMO/project/" Enter
Type "cp $REPO/docs/tapes/demo/mantis.toml $DEMO/project/" Enter
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

# Open the JSON query bar and project a field out of the services array.
# (The path breadcrumb lives in the status bar, which is too low-contrast to
# read in a screenshot; the query bar shows the same feature legibly.)
Ctrl+P
Sleep 1s
Type "json query"
Sleep 1500ms
Enter
Sleep 1s
Type@120ms ".services[]"
Sleep 1s
Enter
Sleep 3s
