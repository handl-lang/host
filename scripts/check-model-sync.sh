#!/usr/bin/env bash
# check-model-sync.sh — fail if live copies of typecheck_model.hnl drift.
#
# The typecheck model source exists in several copies kept identical by
# manual `cp` sync (no generation mechanism).  This script md5sums every
# live copy and exits nonzero on drift.
#
# Covered copies:
#   - host/crates/handl-runtime/tests/fixtures/typecheck_model.hnl
#   - tool-typecheck-handl/src/models/typecheck_model.hnl   (canonical)
#   - tool-typecheck-handl/tests/fixtures/typecheck_model.hnl
#
# Excluded: host/project/design/*/typecheck_model.hnl — stale historical
# snapshots, not live copies.  tool-typecheck-handl also has a Rust-side
# guard (tests/model_fixture_sync.rs) covering its own two copies.
#
# Usage: bash scripts/check-model-sync.sh   (exit 0 = in sync)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LANG_DIR="$(dirname "$ROOT")"

copies=(
  "$ROOT/crates/handl-runtime/tests/fixtures/typecheck_model.hnl"
  "$LANG_DIR/tool-typecheck-handl/src/models/typecheck_model.hnl"
  "$LANG_DIR/tool-typecheck-handl/tests/fixtures/typecheck_model.hnl"
)

ref="$(md5sum "${copies[0]}" | cut -d' ' -f1)"
fail=0
for f in "${copies[@]}"; do
  if [[ ! -f "$f" ]]; then
    echo "MISSING: $f"
    fail=1
    continue
  fi
  sum="$(md5sum "$f" | cut -d' ' -f1)"
  if [[ "$sum" == "$ref" ]]; then
    echo "ok:      $f"
  else
    echo "DRIFT:   $f"
    fail=1
  fi
done

if [[ "$fail" -ne 0 ]]; then
  echo "typecheck_model.hnl copies are out of sync — re-sync with cp from the canonical src/models/typecheck_model.hnl" >&2
fi
exit "$fail"
