#!/usr/bin/env bash
# For each changed non-test src/**.rs file (excluding mod.rs re-export shims,
# lib.rs, and main.rs), asserts a sibling _test.rs is also in the changed set.
# Exits 0 on success; exits 1 listing offenders.
#
# Escape hatch: if any relevant commit message contains "[skip-tests: <reason>]"
# the check is skipped entirely.
#
# Usage (pre-commit — staged files):
#   bash scripts/require-tests.sh
#
# Usage (CI — file list on stdin):
#   cat changed_files.txt | bash scripts/require-tests.sh
set -euo pipefail

# --- Escape hatch -----------------------------------------------------------
# In pre-commit context the message draft lives in .git/COMMIT_EDITMSG.
# In CI context check commits on the branch not yet in origin/main; when the
# checkout is shallow (fetch-depth) and origin/main is not a ref, fall back to
# scanning every commit message reachable within the fetched depth so the
# escape hatch still works for PR/merge-ref checkouts.
skip_token_present() {
    local msg=""
    if [[ -f ".git/COMMIT_EDITMSG" ]]; then
        msg=$(cat .git/COMMIT_EDITMSG)
    elif git rev-parse --verify origin/main >/dev/null 2>&1; then
        msg=$(git log origin/main..HEAD --format=%B 2>/dev/null || true)
    else
        msg=$(git log --format=%B 2>/dev/null || true)
    fi
    echo "$msg" | grep -qE '\[skip-tests:[^]]+\]'
}

if skip_token_present; then
    echo "[require-tests] skip-tests token found — bypassing test-coverage check."
    exit 0
fi

# --- File list --------------------------------------------------------------
# If stdin is a pipe/file, read from it; otherwise use staged files.
if [[ -p /dev/stdin ]] || [[ ! -t 0 ]]; then
    mapfile -t files < <(cat)
else
    mapfile -t files < <(git diff --cached --name-only)
fi

# Build a set of all changed files for fast lookup.
declare -A changed=()
for f in "${files[@]}"; do
    changed["$f"]=1
done

# --- Check ------------------------------------------------------------------
offenders=()

for f in "${files[@]}"; do
    # Only care about Rust source files under src/
    [[ "$f" == src/*.rs ]] || [[ "$f" == src/**/*.rs ]] || continue

    # Skip test files, mod.rs re-export shims, lib.rs, main.rs
    base=$(basename "$f")
    [[ "$base" == *_test.rs ]] && continue
    [[ "$base" == "mod.rs" ]] && continue
    [[ "$base" == "lib.rs" ]] && continue
    [[ "$base" == "main.rs" ]] && continue

    # Derive the expected sibling _test.rs path:
    # src/foo.rs        -> src/foo_test.rs
    # src/app/nav.rs    -> src/app/nav_test.rs
    dir=$(dirname "$f")
    stem="${base%.rs}"
    sibling="${dir}/${stem}_test.rs"

    if [[ -z "${changed[$sibling]:-}" ]]; then
        offenders+=("$f (missing $sibling in diff)")
    fi
done

if ((${#offenders[@]} > 0)); then
    echo "[require-tests] The following source files have no sibling _test.rs in this diff:"
    for o in "${offenders[@]}"; do
        echo "  $o"
    done
    echo ""
    echo "Add or update the corresponding _test.rs file(s), or add"
    echo "  [skip-tests: <reason>]"
    echo "to the commit message if the change is genuinely untestable (UI paint only)."
    exit 1
fi

echo "[require-tests] OK — all changed source files have sibling test coverage."
exit 0
