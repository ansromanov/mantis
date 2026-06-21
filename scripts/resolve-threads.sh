#!/usr/bin/env bash
# Resolve every unresolved review thread on the current branch's PR.
#
# Agents routinely address review comments in code but forget to click the
# "Resolve conversation" button, leaving threads dangling. Run this after
# pushing the fixes so reviewers see what has been handled. Safe to re-run:
# already-resolved threads are skipped. No-ops (exit 0) when the branch has
# no PR.
set -euo pipefail

pr=$(gh pr view --json number -q .number 2>/dev/null) || {
  echo "[resolve-threads] no PR for the current branch; nothing to do."
  exit 0
}

repo=$(gh repo view --json nameWithOwner -q .nameWithOwner)
owner=${repo%/*}
name=${repo#*/}

# Collect unresolved review-thread node IDs across all pages. gh drives
# pagination via the `$endCursor` variable + pageInfo block below.
ids=$(gh api graphql --paginate -F owner="$owner" -F name="$name" -F pr="$pr" \
  -f query='
    query($owner:String!, $name:String!, $pr:Int!, $endCursor:String) {
      repository(owner:$owner, name:$name) {
        pullRequest(number:$pr) {
          reviewThreads(first:100, after:$endCursor) {
            pageInfo { hasNextPage endCursor }
            nodes { id isResolved }
          }
        }
      }
    }' \
  --jq '.data.repository.pullRequest.reviewThreads.nodes[]
        | select(.isResolved == false) | .id')

if [[ -z "$ids" ]]; then
  echo "[resolve-threads] no unresolved threads on PR #$pr."
  exit 0
fi

count=0
while IFS= read -r id; do
  [[ -z "$id" ]] && continue
  gh api graphql -F id="$id" -f query='
    mutation($id:ID!) {
      resolveReviewThread(input:{threadId:$id}) { thread { id } }
    }' >/dev/null
  count=$((count + 1))
done <<< "$ids"

echo "[resolve-threads] resolved $count thread(s) on PR #$pr."
