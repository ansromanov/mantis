#!/usr/bin/env bash
# Generate a dated benchmark report with system details and git commit.
# Usage: ./scripts/bench-report.sh [cargo bench args...]
set -euo pipefail

OUT_DIR="${TV_BENCH_DIR:-bench-results}"
mkdir -p "$OUT_DIR"

TIMESTAMP=$(date -u +"%Y%m%dT%H%M%SZ")
GIT_COMMIT=$(git rev-parse HEAD 2>/dev/null || echo "unknown")
GIT_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "unknown")
GIT_COMMIT_MSG=$(git log -1 --format=%s 2>/dev/null || echo "")

OS=$(uname -s)
KERNEL=$(uname -r)
ARCH=$(uname -m)

if [ "$OS" = "Darwin" ]; then
    CPU=$(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo "unknown")
    CORES=$(sysctl -n hw.logicalcpu 2>/dev/null || echo "unknown")
    MEM_BYTES=$(sysctl -n hw.memsize 2>/dev/null || echo "unknown")
else
    CPU=$(grep -m1 "model name" /proc/cpuinfo 2>/dev/null | cut -d: -f2 | xargs || echo "unknown")
    CORES=$(nproc 2>/dev/null || echo "unknown")
    MEM_BYTES=$(free -b 2>/dev/null | awk '/^Mem:/{print $2}' || echo "unknown")
fi

OUT_FILE="$OUT_DIR/bench-${GIT_COMMIT:0:8}-${TIMESTAMP}.txt"

{
    echo "============================================================"
    echo " tree-viewer benchmark report"
    echo "============================================================"
    printf " %-20s %s\\n" "Date:"       "$(date -u)"
    printf " %-20s %s\\n" "Git commit:" "$GIT_COMMIT"
    printf " %-20s %s\\n" "Git branch:" "$GIT_BRANCH"
    printf " %-20s %s\\n" "Git message:" "$GIT_COMMIT_MSG"
    echo ""
    printf " %-20s %s\\n" "OS:"         "$OS $KERNEL $ARCH"
    printf " %-20s %s\\n" "CPU:"        "$CPU"
    printf " %-20s %s\\n" "Cores:"      "$CORES"
    printf " %-20s %s\\n" "Memory:"     "$MEM_BYTES bytes"
    echo "============================================================"
    echo ""
    echo "command: cargo bench $*"
    echo ""
} | tee "$OUT_FILE"

cargo bench "$@" 2>&1 | tee -a "$OUT_FILE"

echo ""
echo "Report saved to: $OUT_FILE"
