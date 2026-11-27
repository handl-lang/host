#!/usr/bin/env bash
# gate-run.sh — the repo's gate entry point for fleet runners
# (ci-watch.sh, bupha script runner, manual use).
#
# Runs the workflows/gate.yaml campaign against THIS checkout and
# exits 0 iff the run verdict is "pass". `peregrin campaign run`
# itself always exits 0 — the verdict is .ci-result.json's
# `.verdict` field, so this wrapper is what callers should exec.
#
#   bash scripts/gate-run.sh        # gate this checkout
#
# Env passthrough: GATE_REPO/GATE_RESULTS default to $PWD-based
# values (override to gate a different checkout); CI_JOBS,
# HANDL_TOKEN, etc. flow through to scripts/ci.sh inside the
# campaign's gate.run node.
set -uo pipefail

cd "$(dirname "$0")/.." && REPO="$(pwd)"

export PATH="$HOME/.local/bin:$HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin"
export GATE_REPO="${GATE_REPO:-$REPO}"
export GATE_RESULTS="${GATE_RESULTS:-$REPO/.gate-results.jsonl}"

peregrin campaign run workflows/gate.yaml --result .ci-result.json
rc=$?
[ "$rc" -eq 0 ] || exit "$rc"

verdict="$(jq -r '.verdict // "error"' .ci-result.json 2>/dev/null || echo error)"
echo "gate-run: verdict=$verdict ($GATE_REPO)"
[ "$verdict" = "pass" ]
