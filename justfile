default:
    @just --list

# install git hooks and required tools (run once after cloning)
setup:
    cargo install cargo-nextest --locked
    pre-commit install

# start a new feature branch from latest origin/main (e.g. just new my-feature)
# refuses to branch when the current branch already has an open PR — fixing
# review comments must push to that PR, not spawn a new branch. Override for
# genuinely unrelated work with: ALLOW_NESTED=1 just new my-feature
new branch:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ "${ALLOW_NESTED:-0}" != "1" ]] && pr=$(gh pr view --json number -q .number 2>/dev/null); then
        echo "[new] BLOCKED: current branch has open PR #$pr." >&2
        echo "      Fixing review comments? Don't branch — push to this branch then 'just resolve-threads'." >&2
        echo "      Genuinely starting unrelated work? Re-run: ALLOW_NESTED=1 just new {{branch}}" >&2
        exit 1
    fi
    cargo install cargo-nextest --locked
    git fetch origin
    git checkout -b {{branch}} origin/main
    pre-commit install

# check out an existing PR's branch to push fixes (never branch for fixes)
# usage: just fix 240
fix pr:
    gh pr checkout {{pr}}
    pre-commit install

# safe push before opening a PR: fetch latest main, rebase, then push
pr:
    git fetch origin
    git rebase origin/main
    git push -u origin HEAD --force-with-lease

# resolve every unresolved review thread on the current branch's PR
resolve-threads:
    ./scripts/resolve-threads.sh

# end-to-end ship of the current branch: rebase onto fresh main, fmt, related
# tests, push, then open a PR that closes the given issue (or update the existing
# PR + resolve threads).
# usage: just ship 239
# PR body: defaults to one bullet per branch commit + the Closes directive.
# Override with a written summary:  PR_BODY="Why + what." just ship 239
ship issue:
    #!/usr/bin/env bash
    set -euo pipefail
    # 1. ALWAYS rebase onto fresh origin/main before anything else. Tests then run
    #    against the rebased tree; rebase fails loudly on conflicts so you resolve.
    git fetch origin
    git rebase origin/main
    cargo fmt --all
    just test-pr
    git push -u origin HEAD --force-with-lease
    if gh pr view --json number -q .number >/dev/null 2>&1; then
        echo "[ship] existing PR updated; resolving addressed review threads"
        ./scripts/resolve-threads.sh || true
    else
        # 2. ALWAYS open the PR with a descriptive body. Prefer a written summary
        #    via PR_BODY; otherwise derive one bullet per commit on the branch.
        summary="${PR_BODY:-$(git log origin/main..HEAD --reverse --format='- %s')}"
        if [[ -z "${summary//[[:space:]]/}" ]]; then
            echo "[ship] ABORT: empty PR body. Set PR_BODY=\"...\" or add commits." >&2
            exit 1
        fi
        gh pr create \
            --title "$(git log -1 --format=%s)" \
            --body "$(printf '%s\n\nCloses #%s\n' "$summary" "{{issue}}")"
    fi

# build debug binary
build:
    cargo build

# build release binary (re-signs on macOS after strip invalidates the linker signature)
release:
    cargo build --release
    {{ if os() == "macos" { "codesign --force -s - target/release/mantis" } else { "" } }}

# run with optional args (e.g. just run /some/path)
run *args:
    cargo run -- {{args}}

# build release, copy mantis to ~/.cargo/bin, and install default themes
install: release
    #!/usr/bin/env sh
    set -e
    ext=""
    themes_dir="${XDG_CONFIG_HOME:-$HOME/.config}/mantis/themes"
    case "$(uname -s 2>/dev/null)" in
        CYGWIN*|MINGW*|MSYS*)
            ext=".exe"
            themes_dir="${APPDATA}/mantis/themes"
            ;;
    esac
    cp "target/release/mantis${ext}" "${CARGO_HOME:-$HOME/.cargo}/bin/mantis${ext}"
    [ "$(uname -s 2>/dev/null)" = "Darwin" ] && codesign --force -s - "${CARGO_HOME:-$HOME/.cargo}/bin/mantis"
    mkdir -p "${themes_dir}"
    cp themes/*.toml "${themes_dir}/"

# run tests
test *args:
    cargo test {{args}}

# run automated E2E tests (integration tests + whole-binary TUI smoke test)
test-e2e:
    cargo test --test e2e_tests
    python3 scripts/ci-e2e.py

# run only tests related to files changed vs origin/main (fast path for PRs)
# falls back to the full suite when broad files (Cargo.toml, lib.rs, …) change
test-pr:
    #!/usr/bin/env bash
    set -euo pipefail
    changed=$( { git diff --name-only origin/main...HEAD; git diff --name-only; git diff --name-only --cached; } | sort -u )
    # Gate first: every changed source module needs a sibling _test.rs in the diff
    # (escape hatch: "[skip-tests: <reason>]" in a commit message). Fails here, in
    # the agent's own ship loop, instead of only at commit-time or in CI.
    echo "$changed" | bash scripts/require-tests.sh
    filterset=$(echo "$changed" | bash scripts/related-tests.sh)
    if [[ "$filterset" == "__ALL__" ]]; then
        echo "[test-pr] broad change detected — skipping (run 'cargo nextest run' manually if needed)"
        exit 0
    else
        echo "[test-pr] running related tests: $filterset"
        cargo nextest run -E "$filterset"
    fi

# type-check without building
check:
    cargo check

# lint with clippy
clippy:
    cargo clippy

# run performance benchmarks
bench *args:
    cargo bench {{args}}

# run benchmarks and save a dated report with system details + git commit
bench-report *args:
    ./scripts/bench-report.sh {{args}}

# generate shell completions and man page into completions/ and man/ directories
generate-completions: build
    #!/usr/bin/env sh
    set -eu
    mkdir -p completions man
    for shell in bash zsh fish powershell; do
      target/debug/mantis --completions "$shell" > "completions/mantis.${shell}" 2>/dev/null
      echo "generated completions/mantis.${shell}"
    done
    # zsh convention: file is named _mantis
    cp completions/mantis.zsh completions/_mantis
    target/debug/mantis --print-man-page > man/mantis.1 2>/dev/null
    echo "generated man/mantis.1"

# remove build artifacts
clean:
    cargo clean

# publish a GitHub release for the current version
publish:
    gh workflow run release.yml --ref main
