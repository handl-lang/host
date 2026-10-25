#!/usr/bin/env bash
# verify-all.sh — workspace-level verification across the handl-lang
# sibling repos (ADR-0128 layout: one git repo per directory, all sharing
# the workspace root that contains this checkout's parent).
#
# For every sibling directory containing a Cargo.toml:
#
#   1. stale-pin audit — git deps pinned to `rev = "…"` (the release-time
#      shape from scripts/pin-git-revs.py) are LISTED, never a failure;
#      the float-on-main policy (branch = "main") is the expected dev
#      state.
#   2. lib tests — `cargo test --workspace --lib -- --test-threads=1`
#      (serial inside each repo; repos themselves are swept serially —
#      the box is memory-bound, see AGENTS.md). `--locked` whenever the
#      repo commits a Cargo.lock so resolved sibling shas are honored.
#      `cargo update` runs ONLY under --update-locks.
#
# Plus hnl-level checks first: `hnl version`, `hnl ext list` sanity, and
# the full tests/smoke suite via scripts/smoke.sh.
#
# Repos with no #[test] under src/|crates/ are reported `no-tests`
# (skipped, not failed). All artifacts share the configured target dir
# (~/.cargo/config.toml already points CARGO at ~/.cargo/shared-target).
#
#   scripts/verify-all.sh [options]
#
#     --root DIR        workspace root (default: parent of this checkout)
#     --repos REGEX     only repos whose dir name matches
#     --skip REGEX      exclude repos whose dir name matches
#     --update-locks    `cargo update` each repo before testing
#     --timeout SECS    per-repo test timeout (default 900)
#     --jobs N          CARGO_BUILD_JOBS (default 2 — memory-bound box)
#     --hnl PATH        hnl binary (default ~/.cargo/shared-target/debug/hnl)
#     --no-hnl          skip hnl-level checks
#     --no-smoke        skip the smoke suite inside the hnl checks
#     --report FILE     append the tally table to FILE as well
#     -h, --help        this text
#
# Typical local runs:
#   scripts/verify-all.sh --skip '^host$'      # sweep siblings, not the
#                                              # ~19 min host suite
#   scripts/verify-all.sh --repos '^lib-'      # one repo family
#
# Cron / periodic invocation (the box serializes cargo anyway):
#   0 4 * * 6  cd ~/GitHub/handl-lang/host && bash scripts/verify-all.sh \
#              --report verify-all-$(date +\%F).log
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
HOST_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ROOT="${HANDL_WORKSPACE:-$(dirname "$HOST_DIR")}"

REPOS_RE="."; SKIP_RE='^$'
UPDATE_LOCKS=0
TIMEOUT="${VERIFY_TIMEOUT:-900}"
JOBS="${VERIFY_JOBS:-2}"
HNL="${HNL:-$HOME/.cargo/shared-target/debug/hnl}"
DO_HNL=1; DO_SMOKE=1
REPORT=""

while [ $# -gt 0 ]; do
  case "$1" in
    --root) ROOT="$2"; shift 2 ;;
    --repos) REPOS_RE="$2"; shift 2 ;;
    --skip) SKIP_RE="$2"; shift 2 ;;
    --update-locks) UPDATE_LOCKS=1; shift ;;
    --timeout) TIMEOUT="$2"; shift 2 ;;
    --jobs) JOBS="$2"; shift 2 ;;
    --hnl) HNL="$2"; shift 2 ;;
    --no-hnl) DO_HNL=0; shift ;;
    --no-smoke) DO_SMOKE=0; shift ;;
    --report) REPORT="$2"; shift 2 ;;
    -h|--help) sed -n '2,55p' "$0"; exit 0 ;;
    *) echo "verify-all: unknown arg: $1" >&2; exit 2 ;;
  esac
done

[ -d "$ROOT" ] || { echo "verify-all: workspace root missing: $ROOT" >&2; exit 2; }

export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_TEST_DEBUG=0
export CARGO_NET_GIT_FETCH_WITH_CLI=true
export CARGO_BUILD_JOBS="$JOBS"

LOGDIR="$(mktemp -d -t verify-all.XXXXXX)"
trap 'rm -rf "$LOGDIR"' EXIT

ROWS=()
PINNED=()
FAILURES=()
C_PASS=0; C_FAIL=0; C_SKIP=0; C_NOTESTS=0; C_TIMEOUT=0; C_BUILDFAIL=0; C_HNL=0; C_HNL_FAIL=0

row() { # repo status tests secs note
  ROWS+=("$(printf '%-28s %-11s %7s %7s  %s' "$1" "$2" "$3" "$4" "$5")")
}

hnl_check() { # label cmd…
  local label="$1"; shift
  local out rc
  out="$("$@" 2>&1)"; rc=$?
  if [ $rc -eq 0 ]; then
    C_HNL=$((C_HNL + 1)); row "hnl: $label" pass "-" "-" ""
  else
    C_HNL_FAIL=$((C_HNL_FAIL + 1)); FAILURES+=("hnl: $label")
    row "hnl: $label" fail "-" "-" "$(printf '%s' "$out" | head -1)"
  fi
}

# ---------------------------------------------------------------------
# hnl-level checks — the dev toolchain from host-cli + ext-* cdylibs.
# ---------------------------------------------------------------------
if [ "$DO_HNL" = 1 ]; then
  if [ ! -x "$HNL" ]; then
    C_HNL_FAIL=$((C_HNL_FAIL + 1)); FAILURES+=("hnl: binary")
    row "hnl: binary" fail "-" "-" "not executable: $HNL"
  else
    hnl_check "version" "$HNL" version
    out="$("$HNL" ext list 2>&1)"; rc=$?
    missing=""
    for cmd in typecheck run test; do
      printf '%s' "$out" | grep -qw "$cmd" || missing="$missing $cmd"
    done
    if [ $rc -eq 0 ] && [ -z "$missing" ]; then
      C_HNL=$((C_HNL + 1)); row "hnl: ext list" pass "-" "-" "typecheck run test present"
    else
      C_HNL_FAIL=$((C_HNL_FAIL + 1)); FAILURES+=("hnl: ext list")
      row "hnl: ext list" fail "-" "-" "rc=$rc missing:$missing"
    fi
    if [ "$DO_SMOKE" = 1 ]; then
      out="$(bash "$SCRIPT_DIR/smoke.sh" --hnl "$HNL" 2>&1)"; rc=$?
      tally="$(printf '%s' "$out" | grep -oE '[0-9]+ passed' | tail -1 | cut -d' ' -f1)"
      if [ $rc -eq 0 ]; then
        C_HNL=$((C_HNL + 1)); row "hnl: smoke" pass "$tally" "-" "tests/smoke"
      else
        C_HNL_FAIL=$((C_HNL_FAIL + 1)); FAILURES+=("hnl: smoke")
        row "hnl: smoke" fail "${tally:-?}" "-" "$(printf '%s' "$out" | grep FAIL | head -1)"
      fi
    fi
  fi
fi

# ---------------------------------------------------------------------
# Repo sweep.
# ---------------------------------------------------------------------
for d in "$ROOT"/*/; do
  name="$(basename "$d")"
  [ -f "$d/Cargo.toml" ] || continue
  printf '%s' "$name" | grep -qE "$REPOS_RE" || continue
  printf '%s' "$name" | grep -qE "$SKIP_RE" && continue

  note=""
  # Stale-pin audit: rev-pinned sibling git deps are listed, not failed.
  if grep -v '^[[:space:]]*#' "$d/Cargo.toml" | grep -q 'rev *= *"'; then
    PINNED+=("$name"); note="rev-pinned"
  fi

  if [ "$UPDATE_LOCKS" = 1 ]; then
    (cd "$d" && cargo update >"$LOGDIR/$name.update.log" 2>&1) \
      || note="${note:+$note; }update-failed"
  fi

  has_lock=0; [ -f "$d/Cargo.lock" ] && has_lock=1
  [ "$has_lock" = 0 ] && note="${note:+$note; }no-lockfile"

  # Lib tests present? (unit tests inside lib targets only — `--lib`.)
  if ! grep -rql --include='*.rs' '#\[test\]\|#\[cfg *( *test *)' \
        "$d/src" "$d/crates" 2>/dev/null; then
    C_NOTESTS=$((C_NOTESTS + 1)); C_SKIP=$((C_SKIP + 1))
    row "$name" no-tests "-" "-" "$note"
    continue
  fi

  lockflag=""
  [ "$has_lock" = 1 ] && [ "$UPDATE_LOCKS" = 0 ] && lockflag="--locked"
  start=$(date +%s)
  (cd "$d" && timeout "$TIMEOUT" cargo test --workspace --lib $lockflag \
      -- --test-threads=1) >"$LOGDIR/$name.log" 2>&1
  rc=$?
  secs=$(( $(date +%s) - start ))
  ntests="$(grep -oE '[0-9]+ passed' "$LOGDIR/$name.log" | awk '{s+=$1} END{print s+0}')"

  if [ $rc -eq 0 ]; then
    C_PASS=$((C_PASS + 1)); row "$name" pass "$ntests" "${secs}s" "$note"
  elif [ $rc -eq 124 ]; then
    C_TIMEOUT=$((C_TIMEOUT + 1)); FAILURES+=("$name")
    row "$name" timeout "$ntests" "${secs}s" ">$TIMEOUT s $note"
  elif grep -q 'test result:' "$LOGDIR/$name.log"; then
    C_FAIL=$((C_FAIL + 1)); FAILURES+=("$name")
    row "$name" fail "$ntests" "${secs}s" "$(grep -E 'FAILED|error\[' "$LOGDIR/$name.log" | head -1 | cut -c1-60) $note"
  else
    C_BUILDFAIL=$((C_BUILDFAIL + 1)); FAILURES+=("$name")
    row "$name" build-fail "-" "${secs}s" "$(grep -E '^error' "$LOGDIR/$name.log" | head -1 | cut -c1-60) $note"
  fi
done

# ---------------------------------------------------------------------
# Tally.
# ---------------------------------------------------------------------
emit() {
  echo
  printf '%-28s %-11s %7s %7s  %s\n' "repo" "status" "tests" "time" "note"
  printf '%-28s %-11s %7s %7s  %s\n' "----------------------------" "-----------" "-------" "-------" "------------------------------"
  ((${#ROWS[@]})) && printf '%s\n' "${ROWS[@]}"
  echo
  echo "repos:  $C_PASS passed, $C_FAIL test-fail, $C_BUILDFAIL build-fail," \
       "$C_TIMEOUT timeout, $C_NOTESTS no-tests"
  echo "hnl:    $C_HNL passed, $C_HNL_FAIL failed"
  if ((${#PINNED[@]})); then
    echo "stale-pin repos (rev-pinned git deps — listed, not failed):"
    printf '  %s\n' "${PINNED[@]}"
  else
    echo "stale-pin repos: none (all sibling deps float on branch = \"main\")"
  fi
  if ((${#FAILURES[@]})); then
    echo "failures:"; printf '  %s\n' "${FAILURES[@]}"
  fi
}

if [ -n "$REPORT" ]; then emit | tee "$REPORT"; else emit; fi

# Keep the per-repo logs if anything failed — move them out of the
# trap-cleaned tmpdir first.
if ((${#FAILURES[@]})); then
  keep="${TMPDIR:-/tmp}/verify-all-logs-$(date +%s)"
  cp -r "$LOGDIR" "$keep"
  echo "logs kept at: $keep" | tee -a ${REPORT:-/dev/null}
  exit 1
fi
exit 0
