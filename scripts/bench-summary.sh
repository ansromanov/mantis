#!/usr/bin/env bash
# Summarize criterion benchmark results as a markdown table.
#
# Reads target/criterion/**/new/estimates.json (median time per benchmark) and,
# when a baseline from a previous run was restored before `cargo bench`,
# criterion's change/estimates.json (relative drift). Emits the table to stdout
# so callers can tee it into $GITHUB_STEP_SUMMARY and the bench-results
# artifact. Median regressions beyond BENCH_REGRESSION_THRESHOLD percent
# (default 10) get a warning row and a ::warning:: annotation; the script never
# fails the job — shared runners are too noisy for a hard gate.
#
# Usage: ./scripts/bench-summary.sh [criterion-dir]
set -euo pipefail

CRIT_DIR="${1:-${MANTIS_CRITERION_DIR:-target/criterion}}"
THRESHOLD_PCT="${BENCH_REGRESSION_THRESHOLD:-10}"

if [ ! -d "$CRIT_DIR" ]; then
    echo "error: no criterion results at $CRIT_DIR" >&2
    exit 1
fi

estimates=$(find "$CRIT_DIR" -type f -path '*/new/estimates.json' | sort)
if [ -z "$estimates" ]; then
    echo "error: no */new/estimates.json under $CRIT_DIR" >&2
    exit 1
fi

echo "## Benchmark recap"
echo ""
echo "| Benchmark | Median | vs previous run | Status |"
echo "|---|---:|---:|:---|"

regressions=0
improvements=0
compared=0

while IFS= read -r est; do
    bench_dir=$(dirname "$(dirname "$est")")
    id=${bench_dir#"$CRIT_DIR"/}

    median_ns=$(jq -r '.median.point_estimate' "$est")
    human=$(awk -v ns="$median_ns" 'BEGIN {
        if (ns < 1e3)      printf "%.1f ns", ns;
        else if (ns < 1e6) printf "%.2f us", ns / 1e3;
        else if (ns < 1e9) printf "%.2f ms", ns / 1e6;
        else               printf "%.2f s",  ns / 1e9;
    }')

    change_file="$bench_dir/change/estimates.json"
    if [ -f "$change_file" ]; then
        compared=$((compared + 1))
        pct=$(jq -r '.median.point_estimate * 100' "$change_file")
        pct_fmt=$(awk -v p="$pct" 'BEGIN { printf "%+.1f%%", p }')
        verdict=$(awk -v p="$pct" -v t="$THRESHOLD_PCT" 'BEGIN {
            if (p > t)       print "regressed";
            else if (p < -t) print "improved";
            else             print "ok";
        }')
        case "$verdict" in
            regressed)
                regressions=$((regressions + 1))
                status=":warning: regressed"
                echo "::warning::benchmark '$id' median regressed by $pct_fmt (threshold ${THRESHOLD_PCT}%)" >&2
                ;;
            improved)
                improvements=$((improvements + 1))
                status=":rocket: improved"
                ;;
            *)
                status=":white_check_mark: within noise"
                ;;
        esac
    else
        pct_fmt="&ndash;"
        status="no baseline"
    fi

    echo "| \`$id\` | $human | $pct_fmt | $status |"
done <<< "$estimates"

echo ""
if [ "$compared" -eq 0 ]; then
    echo "_No baseline from a previous run was available; drift comparison skipped._"
else
    echo "_Compared $compared benchmark(s) against the previous run:" \
         "$regressions regression(s), $improvements improvement(s)" \
         "beyond the ${THRESHOLD_PCT}% noise threshold._"
fi
