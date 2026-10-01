#!/usr/bin/env bash
# Print committed and working-tree paths changed from the base, one per line.
# Usage: scripts/changed-files.sh [base-ref]
set -euo pipefail

base_ref="${1:-origin/main}"
{
    if git rev-parse --verify --quiet "${base_ref}^{commit}" >/dev/null; then
        git diff --name-only "${base_ref}...HEAD"
    else
        echo "[changed-files] base ref '$base_ref' is unavailable; listing working-tree changes only" >&2
    fi
    git diff --name-only
    git diff --name-only --cached
} | sort -u
