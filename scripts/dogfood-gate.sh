#!/usr/bin/env bash
# Dogfood CI gate: hnl fmt/lint/typecheck/test over the canonical .hnl corpus.
# Non-canonical (arrow/contract dialect) .hnl files are filtered out before the
# canonical tools see them, mirroring plugin-ext-refinement::detect_dialect.
set -euo pipefail

cargo build --release -p handl-host-cli
HNL=./target/release/hnl
status=0

# Equivalent of plugin-ext-refinement::detect_dialect for shell-side filtering.
# Returns 0 if the source looks canonical, 1 for contract or arrow dialect.
is_canonical() {
  local f=$1
  awk '
  function trim(s) { gsub(/^[[:space:]]+|[[:space:]]+$/, "", s); return s }
  function strip_comment(s) { gsub(/--.*$/, "", s); return trim(s) }
  function top_level(s, t) {
    if (s ~ /^[[:space:]]*$/) return ""
    if (s ~ /^[[:space:]]/) return ""
    return strip_comment(s)
  }
  BEGIN { first=1; arrow=0 }
  {
    tl = top_level($0)
    if (first && tl != "") {
      if (tl ~ /^axiom /) { exit 1 }
      first = 0
    }
    if (tl ~ /^(cap|op|handl|tag) /) { arrow = 1 }
    code = strip_comment($0)
    if (code != "") {
      if (index(code, "arr(") || index(code, "first(") || index(code, "choice(") || index(code, "loop(") || index(code, "comp(") || index(code, "app(") || index(code, "|||") || index(code, "rec {"))
        { arrow = 1 }
    }
  }
  END { exit (arrow ? 1 : 0) }
  ' "$f"
}

# Known non-canonical / dialect files that the heuristic detector does not
# catch on its own. Keep this list short and remove entries as detection
# improves.
SKIP_LIST=(
  "handl-packages/collections/array/first.hnl"
  "handl-packages/collections/array.hnl"
)

is_skipped() {
  local f=$1
  for skip in "${SKIP_LIST[@]}"; do
    if [[ "$f" == "$skip" ]]; then
      return 0
    fi
  done
  return 1
}

# Collect the canonical file list once.
canonical_files=()
while IFS= read -r -d '' f; do
  if is_canonical "$f" && ! is_skipped "$f"; then
    canonical_files+=("$f")
  fi
done < <(find handl-packages examples -name '*.hnl' -print0 | sort -z)

echo "Canonical .hnl files: ${#canonical_files[@]}"

echo
echo "== hnl fmt --check =="
fmt_status=0
for f in "${canonical_files[@]}"; do
  if ! "$HNL" fmt --check "$f"; then
    fmt_status=1
    status=1
  fi
done
if [ "$fmt_status" -eq 0 ]; then
  echo "PASS: fmt"
else
  echo "FAIL: fmt"
fi

echo
echo "== hnl lint =="
lint_status=0
for f in "${canonical_files[@]}"; do
  if ! "$HNL" lint "$f"; then
    lint_status=1
    status=1
  fi
done
if [ "$lint_status" -eq 0 ]; then
  echo "PASS: lint"
else
  echo "FAIL: lint"
fi

echo
echo "== hnl typecheck =="
tc_status=0
if [ "${#canonical_files[@]}" -gt 0 ]; then
  if ! "$HNL" typecheck "${canonical_files[@]}"; then
    tc_status=1
    status=1
  fi
fi
if [ "$tc_status" -eq 0 ]; then
  echo "PASS: typecheck"
else
  echo "FAIL: typecheck"
fi

echo
echo "== hnl test =="
test_status=0
if [ "${#canonical_files[@]}" -gt 0 ]; then
  if ! "$HNL" test "${canonical_files[@]}"; then
    test_status=1
    status=1
  fi
else
  echo "SKIP: no canonical test files"
fi
if [ "$test_status" -eq 0 ]; then
  echo "PASS: test"
else
  echo "FAIL: test"
fi

echo
if [ "$status" -eq 0 ]; then
  echo "All dogfood gate phases passed."
else
  echo "One or more dogfood gate phases failed."
fi

exit "$status"
