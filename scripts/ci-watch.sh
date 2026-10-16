#!/usr/bin/env bash
# ci-watch.sh — poll origin/main and run the Dagger CI pipeline on new commits.
#
# A trigger, not a CI server: no UI, no state beyond the last-seen sha.
# Run it anywhere that has docker + the dagger CLI (a $5 VPS, a spare box,
# a tmux on your laptop). Logs append to $CI_LOG_FILE.
#
#   scripts/ci-watch.sh                  # poll every 60 s
#   CI_SHARDS=8 CI_JOBS=2 ./scripts/ci-watch.sh
#   HANDL_TOKEN=... ./scripts/ci-watch.sh   # needed while deps are private
#   CI_POST_STATUS=1 ./scripts/ci-watch.sh  # also post GitHub commit statuses
#
# Env:
#   CI_INTERVAL     seconds between polls            (default 60)
#   CI_SHARDS       nextest partitions               (default 1 — scale out
#                   here: the suite is memory-bound, not CPU-bound)
#   CI_JOBS         -j per shard container           (default 2)
#   CI_DOGFOOD_JOBS HNL_DOGFOOD_JOBS inside dogfood  (default 0 = nproc)
#   HANDL_TOKEN     GitHub token for private git deps + cli fetch
#   CI_POST_STATUS  1 = POST commit status via `gh api`
#   CI_LOG_FILE     append target                    (default ci-watch.log
#                   next to this repo)
#   CI_ONCE         1 = run once for current origin/main, then exit
set -uo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
INTERVAL="${CI_INTERVAL:-60}"
SHARDS="${CI_SHARDS:-1}"
JOBS="${CI_JOBS:-2}"
DOGFOOD_JOBS="${CI_DOGFOOD_JOBS:-0}"
POST_STATUS="${CI_POST_STATUS:-0}"
LOG_FILE="${CI_LOG_FILE:-$REPO_DIR/../ci-watch.log}"
WORKTREE_BASE="${CI_WORKTREE_BASE:-/tmp/handl-ci}"

log() { printf '[%s] %s\n' "$(date -Is)" "$*" | tee -a "$LOG_FILE"; }

post_status() { # state description
  [ "$POST_STATUS" = "1" ] || return 0
  gh api "repos/handl-lang/platform/statuses/$1" \
    -f state="$2" -f context="ci/dagger" -f description="$3" \
    --silent 2>/dev/null || log "status post failed ($2 $3)"
}

# Optional coverage upload (CI_COVERAGE=1): reuses the warm dagger caches
# to produce lcov.info, then uploads with the codecov uploader if
# CODECOV_TOKEN is set and a `codecov` binary is on PATH (one-time:
# curl -Os https://uploader.codecov.io/latest/linux/codecov).
upload_coverage() {
  [ "${CI_COVERAGE:-0}" = "1" ] || return 0
  local wt="$1" sha="$2"
  log "coverage export ${sha:0:12}"
  (cd "$wt" && dagger call coverage-report \
      ${HANDL_TOKEN:+--token env://HANDL_TOKEN} \
      export --path "$wt/coverage") >>"$LOG_FILE" 2>&1 || {
    log "coverage export failed"; return 1; }
  if [ -n "${CODECOV_TOKEN:-}" ] && command -v codecov >/dev/null; then
    (cd "$wt" && codecov -t "$CODECOV_TOKEN" -f coverage/lcov.info -C "$sha") \
      >>"$LOG_FILE" 2>&1 || log "codecov upload failed"
  else
    log "coverage exported to $wt/coverage (no codecov upload)"
  fi
}

run_ci() {
  local sha="$1" wt="$WORKTREE_BASE-${sha:0:12}" rc=0
  rm -rf "$wt"
  git -C "$REPO_DIR" worktree prune >>"$LOG_FILE" 2>&1
  git -C "$REPO_DIR" worktree add --detach "$wt" "$sha" >>"$LOG_FILE" 2>&1 || {
    log "worktree add failed for $sha"; return 1; }
  post_status "$sha" pending "ci running (shard=$SHARDS j=$JOBS)"
  log "ci start ${sha:0:12} (shards=$SHARDS jobs=$JOBS)"
  local args=(--shards "$SHARDS" --jobs "$JOBS" --dogfood-jobs "$DOGFOOD_JOBS")
  [ -n "${HANDL_TOKEN:-}" ] && args+=(--token env://HANDL_TOKEN)
  # cli sibling exists on dev machines; on dedicated runners the token
  # path inside dogfood() fetches cli@main instead.
  [ -d "$REPO_DIR/../cli/crates/host-cli" ] \
    && args+=(--cli-source "$REPO_DIR/../cli")
  (cd "$wt" && dagger call ci "${args[@]}") >>"$LOG_FILE" 2>&1 || rc=$?
  if [ "$rc" -eq 0 ]; then
    log "ci PASS ${sha:0:12}"
    post_status "$sha" success "ci passed"
    upload_coverage "$wt" "$sha"
  else
    log "ci FAIL ${sha:0:12} (exit $rc)"
    post_status "$sha" failure "ci failed"
  fi
  git -C "$REPO_DIR" worktree remove --force "$wt" >>"$LOG_FILE" 2>&1
  return "$rc"
}

last=""
log "watching origin/main (interval ${INTERVAL}s)"
while true; do
  git -C "$REPO_DIR" fetch -q origin main 2>>"$LOG_FILE"
  sha="$(git -C "$REPO_DIR" rev-parse origin/main)"
  if [ "$sha" != "$last" ]; then
    [ -n "$last" ] && log "new commit ${sha:0:12}"
    run_ci "$sha"
    last="$sha"
  fi
  [ "${CI_ONCE:-0}" = "1" ] && exit 0
  sleep "$INTERVAL"
done
