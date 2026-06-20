#!/usr/bin/env bash
# Bundled plugin: show git file log on `H` keypress.
#
# Tracks the last-opened file via `on_file_open` events. On `on_keypress`
# with key `"H"`, runs `git log --oneline --color=always -- "$file"`,
# writes the output to a temp file, and issues `open_file` pointing to
# that temp file so the viewer renders the log as a static file.
#
# Temp files use the prefix `/tmp/tv-git-log-` so the plugin can
# recognise and skip its own output and avoid recursion.
#
# Install: add to [plugins] in tv.toml:
#   git-log = { path = "git-log.sh", enabled = true }

set -euo pipefail

TMP_PREFIX="/tmp/tv-git-log"
TMP_PATTERN="${TMP_PREFIX}-$$"
LAST_FILE=""

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
            [[ "$path" != /tmp/tv-*-* ]] && LAST_FILE="$path"
            ;;
        on_keypress)
            key="${line#*\"key\":\"}"
            key="${key%%\"*}"
            if [[ "$key" == "H" && -n "$LAST_FILE" ]]; then
                dir="$(dirname "$LAST_FILE")"
                repo="$(cd "$dir" && git rev-parse --show-toplevel 2>/dev/null)" || continue
                log="$(git -C "$repo" log --oneline --color=always -- "$LAST_FILE" 2>/dev/null)" || continue
                [[ -z "$log" ]] && continue
                tmp="$(mktemp "${TMP_PATTERN}-XXXXXX")"
                printf '%s\n' "$log" > "$tmp"
                printf '{"event":"action","action":"open_file","params":{"path":"%s"}}\n' "$tmp"
            fi
            ;;
        shutdown)
            exit 0
            ;;
    esac
done
