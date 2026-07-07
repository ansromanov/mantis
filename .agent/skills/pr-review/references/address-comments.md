# Phase 4 — Address PR review comments

Fetch all open (unresolved) review threads:
```bash
gh api graphql --paginate \
  -F owner="${REPO%/*}" -F name="${REPO#*/}" -F pr="$PR" \
  -f query='
    query($owner:String!, $name:String!, $pr:Int!, $endCursor:String) {
      repository(owner:$owner, name:$name) {
        pullRequest(number:$pr) {
          reviewThreads(first:100, after:$endCursor) {
            pageInfo { hasNextPage endCursor }
            nodes {
              id
              isResolved
              comments(first:10) {
                nodes { body author { login } path line }
              }
            }
          }
        }
      }
    }' \
  --jq '.data.repository.pullRequest.reviewThreads.nodes[] | select(.isResolved==false)'
```

For each unresolved thread:
1. Read the comment(s) to understand what is requested
2. Apply the fix (edit code, then fmt + clippy as above)
3. Track which threads you addressed

If a comment is ambiguous or contradicts AGENTS.md, note it and skip rather than guessing.
