#!/usr/bin/env bash
# Bundled plugin: show git working-tree diff for tracked files on open.
#
# On `on_file_open`, if the file is tracked by git, runs
# `git diff --color=always HEAD` to produce an ANSI-coloured diff,
# writes it to a temp file, and issues `open_file` pointing to that
# temp file so the viewer renders the diff instead of the file content.
#
# Temp files use the prefix `/tmp/tv-git-diff-` so the plugin can
# recognise and skip its own output and avoid recursion.
#
# Install: add to [plugins] in tv.toml:
#   git-diff = { path = "git-diff.sh", enabled = true }

set -euo pipefail

TMP_PREFIX="/tmp/tv-git-diff"
TMP_PATTERN="${TMP_PREFIX}-$$"

cleanup() {
    rm -f "${TMP_PATTERN}-"* 2>/dev/null
}
trap cleanup EXIT

while IFS= read -r line; do
    event="${line#*\"event\":\"}"
    event="${event%%\"*}"

    case "$event" in
        on_file_open)
            path="${line#*\"path\":\"}"
            path="${path%%\"*}"
            [[ "$path" == "$TMP_PREFIX"* ]] && continue
            [[ -z "$path" || ! -f "$path" ]] && continue
            dir="$(dirname "$path")"
            repo="$(cd "$dir" && git rev-parse --show-toplevel 2>/dev/null)" || continue
            diff="$(git -C "$repo" diff --color=always HEAD -- "$path" 2>/dev/null)" || continue
            [[ -z "$diff" ]] && continue
            tmp="$(mktemp "${TMP_PATTERN}-XXXXXX")"
            printf '%s\n' "$diff" > "$tmp"
            printf '{"event":"action","action":"open_file","params":{"path":"%s"}}\n' "$tmp"
            ;;
        shutdown)
            exit 0
            ;;
    esac
done
