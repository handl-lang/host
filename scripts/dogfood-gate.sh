#!/usr/bin/env bash
# Dogfood CI gate: hnl fmt/lint/typecheck/test over the canonical .hnl corpus.
# Non-canonical (arrow/contract dialect) .hnl files are filtered out before the
# canonical tools see them, mirroring plugin-ext-refinement::detect_dialect.
set -euo pipefail

# ADR-0128 Phase 8: `handl-host-cli` (the `hnl` binary) lives in the
# sibling `cli` repo; build it there. Its git deps resolve via that repo's
# `[patch]` tables back into this checkout, so the build exercises the
# same platform sources.
if [ ! -d ../cli/crates/host-cli ]; then
  echo "error: sibling checkout ../cli not found — clone handl-lang/cli next to this repo" >&2
  exit 1
fi
(cd ../cli && cargo build --release --bin hnl)
HNL=../cli/target/release/hnl


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
      if (index(code, "|||") || code ~ /(^|[^a-zA-Z0-9_.])(arr|first|choice|loop|comp|app)\(/)
        { arrow = 1 }
      if (code ~ /(^|[^a-zA-Z0-9_.])~>/) { arrow = 1 }
      if (code ~ /(^|[^a-zA-Z0-9_.])rec[[:space:]]*\{/) { arrow = 1 }
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
    if [[ "$f" == "$skip" || "$f" == "$skip"/* ]]; then
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
done < <(find handl-packages -name '*.hnl' -print0 | sort -z)

echo "Canonical .hnl files: ${#canonical_files[@]}"

# Parallelism for the slow phases. `nproc` is available on Linux; default
# to 4 on macOS or minimal containers.
JOBS=${HNL_DOGFOOD_JOBS:-$(nproc 2>/dev/null || echo 4)}
# Batch size: enough to amortize the per-process Language/Kit assembly
# (significant for typecheck/lint), small enough to keep many batches
# running in parallel and finish within a reasonable tail.
BATCH_SIZE=12

echo
echo "== hnl fmt --check (jobs: $JOBS) =="
fmt_status=0
if [ "${#canonical_files[@]}" -gt 0 ]; then
  if ! printf '%s\n' "${canonical_files[@]}" | xargs -P "$JOBS" -n 1 "$HNL" fmt --check; then
    fmt_status=1
    status=1
  fi
fi
if [ "$fmt_status" -eq 0 ]; then
  echo "PASS: fmt"
else
  echo "FAIL: fmt"
fi

echo
echo "== hnl lint (jobs: $JOBS, batch: $BATCH_SIZE) =="
lint_status=0
if [ "${#canonical_files[@]}" -gt 0 ]; then
  if ! printf '%s\n' "${canonical_files[@]}" | xargs -P "$JOBS" -n "$BATCH_SIZE" "$HNL" lint; then
    lint_status=1
    status=1
  fi
fi
if [ "$lint_status" -eq 0 ]; then
  echo "PASS: lint"
else
  echo "FAIL: lint"
fi

echo
echo "== hnl typecheck (jobs: $JOBS, batch: $BATCH_SIZE) =="
tc_status=0
if [ "${#canonical_files[@]}" -gt 0 ]; then
  if ! printf '%s\n' "${canonical_files[@]}" | xargs -P "$JOBS" -n "$BATCH_SIZE" "$HNL" typecheck; then
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
echo "== hnl test (jobs: $JOBS, per-file) =="
test_status=0
if [ "${#canonical_files[@]}" -gt 0 ]; then
  # Per-file processes: a single `hnl test` over the corpus accumulates
  # module state per file and gets OOM-killed (~9GB on ~225 files);
  # even 12-file batches at -P8 blew past 11GB. One file per process
  # bounds peak RSS to the worst single file.
  if ! printf '%s\n' "${canonical_files[@]}" | xargs -P "$JOBS" -n 1 "$HNL" test --jobs 4; then
    test_status=1
    status=1
  fi
fi
if [ "$test_status" -eq 0 ]; then
  echo "PASS: test"
else
  echo "FAIL: test"
fi

# ---------------------------------------------------------------------------
# Arrow-dialect packages
#
# The dialect detector above excludes `~>`/`~[n]~>` files from the canonical
# phases, which left the whole server/client stack (concurrent-epoll,
# serve-epoll, http_rfc, fetch-c, ws-c, tls-libssl, ...) outside CI. These
# packages get their own typecheck + test pass. The list is curated: add a
# package here when it joins the maintained stack (the detector cannot yet
# tell an arrow implementation from a signature-only file).
# ---------------------------------------------------------------------------
# ADR-0128 Phase 5: the arrow-dialect stack moved to the `c` and `web`
# repos (platform `handl-packages/` no longer holds them); each repo gates
# its own packages. The mechanism stays for any future in-repo arrow
# package.
ARROW_PACKAGES=(
)

arrow_sources=()
arrow_tests=()
for pkg in "${ARROW_PACKAGES[@]}"; do
  [ -d "$pkg" ] || continue
  while IFS= read -r -d '' f; do
    case "$f" in
      *.test.hnl) arrow_tests+=("$f") ;;
      *) arrow_sources+=("$f") ;;
    esac
  done < <(find "$pkg" -name '*.hnl' -print0 | sort -z)
done

echo
echo "== arrow packages: ${#arrow_sources[@]} sources, ${#arrow_tests[@]} test files =="

echo
echo "== hnl typecheck (arrow, jobs: $JOBS, batch: $BATCH_SIZE) =="
arrow_tc=0
if [ "${#arrow_sources[@]}" -gt 0 ]; then
  if ! printf '%s\n' "${arrow_sources[@]}" "${arrow_tests[@]}" \
      | xargs -P "$JOBS" -n "$BATCH_SIZE" "$HNL" typecheck; then
    arrow_tc=1
    status=1
  fi
fi
if [ "$arrow_tc" -eq 0 ]; then
  echo "PASS: typecheck (arrow)"
else
  echo "FAIL: typecheck (arrow)"
fi

echo
echo "== hnl test (arrow, jobs: $JOBS, per-file, in-file: 1) =="
arrow_test=0
if [ "${#arrow_tests[@]}" -gt 0 ]; then
  # `--jobs 1` inside each file: several of these suites share one fixture
  # process and port (fetch-c's canned server even shuts down on GET /done),
  # so running the cases of a file concurrently makes them fight over it.
  # Files still run in parallel with each other.
  if ! printf '%s\n' "${arrow_tests[@]}" | xargs -P "$JOBS" -n 1 "$HNL" test --jobs 1; then
    arrow_test=1
    status=1
  fi
fi
if [ "$arrow_test" -eq 0 ]; then
  echo "PASS: test (arrow)"
else
  echo "FAIL: test (arrow)"
fi

echo
if [ "$status" -eq 0 ]; then
  echo "All dogfood gate phases passed."
else
  echo "One or more dogfood gate phases failed."
fi

exit "$status"

