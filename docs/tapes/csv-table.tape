# CSV/TSV table view (v0.18). Regenerate with: just docs-screenshots
Output .vhs-out/csv-table.png

Require mantis

Set FontFamily "Menlo"
Set FontSize 16
Set Width 1400
Set Height 820
Set Padding 24

Hide
Type "export REPO=$PWD" Enter
Type "export DEMO=${TMPDIR:-/tmp}/mantis-shot-csv" Enter
Type "rm -rf $DEMO && mkdir -p $DEMO/home $DEMO/project" Enter
Type "cp $REPO/docs/tapes/fixtures/quarterly-usage.csv $DEMO/project/" Enter
Type "cp $REPO/docs/tapes/fixtures/deploy-report.json $DEMO/project/" Enter
Type "cp $REPO/docs/tapes/fixtures/deployment.yaml $DEMO/project/" Enter
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

# Open by name rather than by cursor position: tree order is not something a
# tape should depend on.
Ctrl+T
Sleep 1s
Type "quarterly"
Sleep 1500ms
Enter
Sleep 3s
