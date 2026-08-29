#!/usr/bin/env bash
# Run LLVM source-based test coverage for the handl platform workspace.
#
# Uses `cargo-llvm-cov` (install with: cargo install cargo-llvm-cov).
#
# Usage:
#   scripts/coverage.sh                # HTML + lcov + terminal summary
#   scripts/coverage.sh --summary      # terminal summary only (no files)
#   scripts/coverage.sh --doctests     # include doctests in the report
#   scripts/coverage.sh --fail-under 80   # exit 1 if line coverage < 80%
#   scripts/coverage.sh -p <crate>     # cover a single crate
#   scripts/coverage.sh --clean        # remove stale artifacts first
#
# Outputs:
#   coverage/html/index.html  — browsable annotated-source report
#   coverage/lcov.info        — lcov format (CI / VS Code Coverage Gutters)
#   coverage/summary.txt      — the textual per-crate summary

set -euo pipefail

cd "$(dirname "$0")/.."

OUT_DIR="coverage"
# cargo-llvm-cov creates an `html/` subdir inside --output-dir, so point it
# at $OUT_DIR and reference $OUT_DIR/html/index.html as the entry point.
LCOV_FILE="$OUT_DIR/lcov.info"
SUMMARY_FILE="$OUT_DIR/summary.txt"

SUMMARY_ONLY=0
DOCTESTS=0
FAIL_UNDER=""
CRATE=""
CLEAN=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --summary)   SUMMARY_ONLY=1; shift ;;
    --doctests)  DOCTESTS=1; shift ;;
    --clean)     CLEAN=1; shift ;;
    --fail-under)
      FAIL_UNDER="$2"; shift 2 ;;
    -p|--package)
      CRATE="$2"; shift 2 ;;
    -h|--help)
      sed -n '2,16p' "$0"; exit 0 ;;
    *)
      echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

if ! command -v cargo-llvm-cov >/dev/null 2>&1; then
  echo "error: cargo-llvm-cov not installed." >&2
  echo "  install with: cargo install cargo-llvm-cov" >&2
  exit 1
fi

# Instrumented builds are incompatible with incremental compilation.
export CARGO_INCREMENTAL=0
# A stable temp dir keeps profraw files out of the source tree.
export TMPDIR="${TMPDIR:-/tmp}"

if [[ $CLEAN -eq 1 ]]; then
  echo "==> cleaning stale coverage artifacts"
  cargo llvm-cov clean --workspace
fi

PKG_ARG=()
if [[ -n "$CRATE" ]]; then
  PKG_ARG=(-p "$CRATE")
fi

EXTRA_ARG=()
if [[ $DOCTESTS -eq 1 ]]; then
  EXTRA_ARG+=(--doctests)
fi

if [[ $SUMMARY_ONLY -eq 1 ]]; then
  echo "==> running coverage (summary only)"
  if [[ -n "$CRATE" ]]; then
    cargo llvm-cov "${PKG_ARG[@]}" "${EXTRA_ARG[@]}" --summary-only
  else
    cargo llvm-cov --workspace "${EXTRA_ARG[@]}" --summary-only
  fi
  exit 0
fi

mkdir -p "$OUT_DIR"

echo "==> running tests with coverage instrumentation"
SCOPE_ARG=(--workspace)
if [[ -n "$CRATE" ]]; then
  SCOPE_ARG=()
fi
# Run tests and collect profraw data without emitting a report yet.
cargo llvm-cov "${SCOPE_ARG[@]}" "${PKG_ARG[@]}" "${EXTRA_ARG[@]}" --no-report 2>&1 \
  | tee "$SUMMARY_FILE"

FAIL_ARG=()
if [[ -n "$FAIL_UNDER" ]]; then
  FAIL_ARG=(--fail-under-lines "$FAIL_UNDER")
fi

echo "==> generating lcov report"
cargo llvm-cov report "${SCOPE_ARG[@]}" "${PKG_ARG[@]}" \
  --lcov --output-path "$LCOV_FILE" "${FAIL_ARG[@]}"

echo "==> generating HTML report"
cargo llvm-cov report "${SCOPE_ARG[@]}" "${PKG_ARG[@]}" \
  --html --output-dir "$OUT_DIR"

echo "==> terminal summary"
cargo llvm-cov report "${SCOPE_ARG[@]}" "${PKG_ARG[@]}" --summary-only

echo
echo "coverage report:"
echo "  HTML:    $OUT_DIR/html/index.html"
echo "  lcov:    $LCOV_FILE"
echo "  summary: $SUMMARY_FILE"
