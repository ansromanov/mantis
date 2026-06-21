#!/usr/bin/env bash
# Bundled plugin: comprehensive git support for tv.
#
# Handles all git sub-systems that were previously built into tv's core:
#   - git status (file statuses for tree coloring)
#   - repo info (branch, HEAD, dirty state in status bar)
#   - working-tree diff (on file open for tracked files)
#   - file log (on H keypress)
#   - file blame (on b keypress)
#
# Protocol: receives events on stdin (one JSON object per line) and
# responds with actions on stdout. See docs/src/plugin-development.md.
#
# Install by adding to [plugins] in tv.toml:
#   git-plugin = { path = "git-plugin.sh", enabled = true }

set -euo pipefail

TMP_DIFF_PREFIX="/tmp/tv-git-diff"
TMP_LOG_PREFIX="/tmp/tv-git-log"
TMP_PATTERN="$$"
DIFF_TMP="${TMP_DIFF_PREFIX}-${TMP_PATTERN}"
LOG_TMP="${TMP_LOG_PREFIX}-${TMP_PATTERN}"
LAST_FILE=""
LAST_SEL_FILE=""

cleanup() {
    rm -f "${DIFF_TMP}-"* "${LOG_TMP}-"* 2>/dev/null
}
trap cleanup EXIT

json_escape() {
    printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g; s/\n/\\n/g; s/\r/\\r/g; s/\t/\\t/g'
}

send_action() {
    printf '{"event":"action","action":"%s","params":%s}\n' "$1" "$2"
}

get_repo() {
    local dir="$1"
    (
        cd "$dir" 2>/dev/null || exit 1
        git rev-parse --show-toplevel 2>/dev/null
    ) || return 1
}

send_repo_info() {
    local file="$1"
    local dir
    dir="$(dirname "$file")"
    local repo
    repo="$(get_repo "$dir")" || return

    local branch head dirty state
    branch="$(cd "$repo" && git rev-parse --abbrev-ref HEAD 2>/dev/null)" || branch=""
    head="$(cd "$repo" && git rev-parse --short HEAD 2>/dev/null)" || head=""

    if [ -n "$(cd "$repo" && git status --porcelain 2>/dev/null)" ]; then
        dirty="true"
        state="dirty"
    else
        dirty="false"
        state="clean"
    fi

    if [ -d "$repo/.git/rebase-merge" ] || [ -d "$repo/.git/rebase-apply" ]; then
        state="rebase"
    elif [ -f "$repo/.git/MERGE_HEAD" ]; then
        state="merge"
    fi

    send_action "set_status_bar_git_info" \
        "{\"branch\":\"$(json_escape "$branch")\",\"head\":\"$(json_escape "$head")\",\"dirty\":$dirty,\"state\":\"$(json_escape "$state")\"}"
}

send_file_statuses() {
    local file="$1"
    local dir
    dir="$(dirname "$file")"
    local repo
    repo="$(get_repo "$dir")" || return

    local statuses
    statuses="$(cd "$repo" && git status --porcelain 2>/dev/null)" || return
    [ -z "$statuses" ] && return

    local json="{"
    local first=true

    while IFS= read -r line; do
        [ -z "$line" ] && continue
        local xy="${line:0:2}"
        local path_str="${line:3}"
        if [[ "$path_str" == *" -> "* ]]; then
            path_str="${path_str##* -> }"
        fi
        path_str="${path_str%/}"
        [ -z "$path_str" ] && continue

        local status_str
        case "$xy" in
            "M "|"MM"|" M") status_str="modified" ;;
            "A "|"AM"|" A") status_str="added" ;;
            "D "|" D"|"AD") status_str="deleted" ;;
            "R "|"RM")       status_str="renamed" ;;
            "??")            status_str="untracked" ;;
            "!!")            status_str="ignored" ;;
            "UU"|"AA"|"DD"|"U "|" U") status_str="conflict" ;;
            *) continue ;;
        esac

        local fullpath="$repo/$path_str"
        local escaped_path
        escaped_path="$(json_escape "$fullpath")"

        if [ "$first" = true ]; then
            first=false
            json="${json}\"${escaped_path}\":\"${status_str}\""
        else
            json="${json},\"${escaped_path}\":\"${status_str}\""
        fi
    done <<< "$statuses"

    json="${json}}"
    send_action "set_file_statuses" "$json"
}

send_blame_data() {
    local file="$1"
    if ! command -v python3 &>/dev/null; then
        send_action "show_message" "{\"message\":\"blame requires python3\"}"
        return
    fi
    local result
    result=$(python3 -c "
import subprocess, sys, json, os

file_path = '$file'
dir_path = os.path.dirname(file_path)

try:
    result = subprocess.run(
        ['git', 'blame', '--short', '--', file_path],
        capture_output=True, text=True, timeout=60,
        cwd=dir_path
    )
    if result.returncode != 0:
        print(json.dumps({'path': file_path, 'lines': []}))
        sys.exit(0)
except Exception:
    print(json.dumps({'path': file_path, 'lines': []}))
    sys.exit(0)

lines = result.stdout.splitlines()
# Map blame prefix by 0-based physical line index
blame_map = {}
for bline in lines:
    if not bline:
        continue
    # Format: 'abc1234 (Author Name 2024-01-01)   42) content'
    paren_end = bline.find(') ')
    if paren_end < 0:
        continue
    prefix = bline[:paren_end+2]
    blame_map[len(blame_map)] = prefix

# Get total file line count
try:
    with open(file_path, 'r') as f:
        total = sum(1 for _ in f)
except Exception:
    total = max(blame_map.keys()) + 1 if blame_map else 0

arr = [blame_map.get(i, '') for i in range(total)]
print(json.dumps({'path': file_path, 'lines': arr}))
" 2>/dev/null) || {
        send_action "show_message" "{\"message\":\"blame failed\"}"
        return
    }
    send_action "set_blame_data" "$result"
}

send_diff() {
    local file="$1"
    [[ "$file" == "$TMP_DIFF_PREFIX"* || "$file" == "$TMP_LOG_PREFIX"* ]] && return
    [ ! -f "$file" ] && return

    local dir
    dir="$(dirname "$file")"
    local repo
    repo="$(get_repo "$dir")" || return

    local diff
    diff="$(git -C "$repo" diff --color=always HEAD -- "$file" 2>/dev/null)" || return
    [ -z "$diff" ] && return

    local tmp
    tmp="$(mktemp "${DIFF_TMP}-XXXXXX")"
    printf '%s\n' "$diff" > "$tmp"
    send_action "open_file" "{\"path\":\"$(json_escape "$tmp")\"}"
}

send_log() {
    local file="$1"
    [ -z "$file" ] && return

    local dir
    dir="$(dirname "$file")"
    local repo
    repo="$(get_repo "$dir")" || return

    local log
    log="$(git -C "$repo" log --oneline --color=always -- "$file" 2>/dev/null)" || return
    [ -z "$log" ] && return

    local tmp
    tmp="$(mktemp "${LOG_TMP}-XXXXXX")"
    printf '%s\n' "$log" > "$tmp"
    send_action "open_file" "{\"path\":\"$(json_escape "$tmp")\"}"
}

while IFS= read -r line; do
    event="${line#*\"event\":\"}"
    event="${event%%\"*}"

    case "$event" in
        init)
            # Wait for on_file_open to get the current file path.
            ;;
        on_file_open)
            path="${line#*\"path\":\"}"
            path="${path%%\"*}"
            [ -z "$path" ] && continue

            LAST_FILE="$path"
            send_repo_info "$path"
            send_file_statuses "$path"
            send_diff "$path"
            ;;
        on_selection_change)
            path="${line#*\"path\":\"}"
            path="${path%%\"*}"
            [ -z "$path" ] && continue
            [ "$path" = "$LAST_SEL_FILE" ] && continue
            LAST_SEL_FILE="$path"
            send_file_statuses "$path"
            ;;
        on_keypress)
            key="${line#*\"key\":\"}"
            key="${key%%\"*}"
            case "$key" in
                H)  send_log "$LAST_FILE" ;;
                b)  [ -n "$LAST_FILE" ] && send_blame_data "$LAST_FILE" ;;
            esac
            ;;
        shutdown)
            exit 0
            ;;
    esac
done
