# Kubernetes manifest facts in the status bar (v0.19).
# Regenerate with: just docs-screenshots
#
# Uses a single-document manifest deliberately: mantis currently reports a
# YAML error in the status bar for multi-document files (issue #849; the
# multi-doc fixture manifest.yaml reproduces it), which would put a false
# error in the screenshot.
Output .vhs-out/k8s-facts.png

Require mantis

Set FontFamily "Menlo"
Set FontSize 16
Set Width 1400
Set Height 820
Set Padding 24

Hide
Type "export REPO=$PWD" Enter
Type "export DEMO=${TMPDIR:-/tmp}/mantis-shot-k8s" Enter
Type "rm -rf $DEMO && mkdir -p $DEMO/home $DEMO/project" Enter
Type "cp $REPO/docs/tapes/fixtures/deployment.yaml $DEMO/project/" Enter
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
Type "deployment"
Sleep 1500ms
Enter
Sleep 4s
