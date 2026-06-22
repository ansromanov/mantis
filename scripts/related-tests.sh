#!/usr/bin/env bash
# Reads changed file paths on stdin (one per line) and prints a cargo-nextest
# filterset (the argument to `-E`) selecting only the tests related to those
# changes. Prints the literal token `__ALL__` to request the full suite when a
# change is broad enough that a related subset cannot be trusted.
#
# Mapping rules:
#   src/foo.rs            -> unit tests under module `foo`        (foo::...)
#   src/app/nav.rs        -> unit tests under module `app::nav`
#   src/app/mod.rs        -> unit tests under module `app`
#   src/foo_test.rs       -> unit tests under module `foo`
#   tests/foo_tests.rs    -> integration binary `foo_tests`
#   src/foo.rs (+ tests/foo_tests.rs present) -> also that integration binary
#
# Fallback to __ALL__ on: Cargo.toml/Cargo.lock/tv.toml, .cargo/config.toml,
# src/lib.rs, src/main.rs, or when nothing maps to a test.
set -euo pipefail

join_by() { local sep=$1; shift; local out=${1:-}; shift || true; local x; for x in "$@"; do out="$out$sep$x"; done; printf '%s' "$out"; }

unit_preds=()
bin_preds=()
declare -A seen_unit=() seen_bin=()

add_unit() { local m=$1; [[ -n ${seen_unit[$m]:-} ]] && return 0; seen_unit[$m]=1; unit_preds+=("test(/^${m}(::|\$)/)"); }
add_bin()  { local b=$1; [[ -n ${seen_bin[$b]:-} ]] && return 0; seen_bin[$b]=1; bin_preds+=("binary(${b})"); }

while IFS= read -r f; do
  [[ -z $f ]] && continue
  case "$f" in
    Cargo.toml|Cargo.lock|tv.toml|.cargo/config.toml|.github/workflows/*) echo __ALL__; exit 0 ;;
    src/lib.rs|src/main.rs) echo __ALL__; exit 0 ;;
    src/*.rs)
      m=${f#src/}; m=${m%.rs}
      # When a _test.rs file's stem matches the parent directory name
      # (e.g. src/ui/content/content_test.rs), the tests are declared in
      # the parent module (mod.rs declares `mod tests`), so map to the
      # parent path rather than the nonexistent sibling module.
      _base=${m##*/}; _dir=${m%/*}
      if [[ "$_base" != "$m" && "${_base%_test}" == "${_dir##*/}" ]]; then
        m=$_dir
      else
        m=${m%_test}; m=${m%/mod}
      fi
      seg=${m##*/}
      m=${m//\//::}
      add_unit "$m"
      [[ -f "tests/${seg}_tests.rs" ]] && add_bin "${seg}_tests"
      ;;
    tests/*.rs)
      b=${f#tests/}; b=${b%.rs}
      add_bin "$b"
      ;;
    *) ;; # non-code change; ignored (workflow gates these out upstream)
  esac
done

preds=()
if ((${#unit_preds[@]})); then
  preds+=("binary(tree_viewer) & ( $(join_by ' + ' "${unit_preds[@]}") )")
fi
if ((${#bin_preds[@]})); then
  preds+=("$(join_by ' + ' "${bin_preds[@]}")")
fi

if ((${#preds[@]} == 0)); then echo __ALL__; exit 0; fi
join_by ' + ' "${preds[@]}"
echo
