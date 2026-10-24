#!/usr/bin/env bash
# ci.sh — host CI pipeline for the bupha shell runner.
#
# Migrated from the Dagger module (dagger/src/ci/main.py — kept on disk
# but deprecated): same stages, no containers, no docker. bupha invokes
# this via `runner.kind = "shell"` → `bash -c "bash scripts/ci.sh"` inside
# a detached worktree of the pushed sha; `bupha run` runs it in the main
# checkout. It also works standalone.
#
#   scripts/ci.sh                  # the whole gate: test + coverage-gate + dogfood
#   scripts/ci.sh doctor           # toolchain + layout smoke, no compile
#   scripts/ci.sh test             # nextest partitions (serial) + doctests
#   scripts/ci.sh test-shard I N   # one nextest partition count:I/N
#   scripts/ci.sh doctests         # cargo test --doc --workspace
#   scripts/ci.sh coverage-report  # coverage/ html + lcov + summary
#   scripts/ci.sh coverage-gate    # blocking: tooling-crates line floor
#   scripts/ci.sh dogfood          # build hnl from host-cli, run the corpus
#
# Env:
#   CI_JOBS          nextest -j / libtest threads (default 2 — the suite is
#                    memory-bound on an 11 GiB box; do NOT raise, -j 8 OOMs)
#   CI_SHARDS        nextest partitions, run serially (default 1 — scale
#                    out needs multiple boxes, one box = one partition)
#   CI_BUILD_JOBS    cargo build -j (default 2)
#   CI_DOGFOOD_JOBS  HNL_DOGFOOD_JOBS (default 2 — nproc=16 OOMs; even -P8
#                    on batched `hnl test` blew past 11 GiB)
#   CI_TARGET_DIR    shared cargo target dir
#                    (default <workspace>/.ci-target/host — without it a
#                    detached worktree cold-builds every sha)
#   CI_SKIP_DOGFOOD  1 = never run the dogfood stage in `ci`
#   HANDL_TOKEN / BUPHA_TOKEN  GitHub token for private git deps. Optional
#                    locally: every handl-lang dep is [patch]'d to a
#                    sibling checkout, so cargo never hits github for them.
set -uo pipefail

cd "$(dirname "$0")/.."

JOBS="${CI_JOBS:-2}"
SHARDS="${CI_SHARDS:-1}"
BUILD_JOBS="${CI_BUILD_JOBS:-2}"
DOGFOOD_JOBS="${CI_DOGFOOD_JOBS:-2}"

# Same env block the dagger module set on its containers (debug=0 keeps
# the target dir small enough for a disk-constrained box).
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_TEST_DEBUG=0
# Cargo must shell out to git so the insteadOf credential rewrite and
# ssh/config auth apply to private git deps.
export CARGO_NET_GIT_FETCH_WITH_CLI=true
export CARGO_BUILD_JOBS="$BUILD_JOBS"
# Caps libtest threads inside `cargo test` / `cargo llvm-cov` runs
# (nextest has its own -j).
export RUST_TEST_THREADS="$JOBS"

# ---------------------------------------------------------------------------
# Layout plumbing — what the dagger module did with container mounts +
# dev-link execs, done here with symlinks.
# ---------------------------------------------------------------------------

# Workspace root = parent of the *main* checkout (correct from both the
# checkout and any detached worktree of this repo).
GIT_COMMON="$(git rev-parse --path-format=absolute --git-common-dir)"
MAIN_CHECKOUT="$(dirname "$GIT_COMMON")"
WORKSPACE_ROOT="$(dirname "$MAIN_CHECKOUT")"
IN_WORKTREE=0
[ "$PWD" != "$MAIN_CHECKOUT" ] && IN_WORKTREE=1

# Shared target dir — a detached worktree's own target/ would cold-build
# every sha (~hours + ~8 GB on this box). Runs are serialized by the
# watcher (single worker, latest-wins) and by ci-watch's poll loop, so
# sharing is safe. Prune with `cargo clean` under CI_TARGET_DIR if disk
# gets tight.
export CARGO_TARGET_DIR="${CI_TARGET_DIR:-$WORKSPACE_ROOT/.ci-target/host}"

# The committed [patch] tables are relative paths (../kernel, ../host…),
# including a SELF-patch: `[patch."…/host"] -> ../host` must resolve to
# the checkout under test, or transitive git deps silently build the dev
# checkout's sources instead of the pushed sha. In a detached worktree
# recreate the sibling layout in the worktree's parent: ../host -> this
# worktree, every other sibling -> the real checkout.
ensure_siblings() {
  [ "$IN_WORKTREE" = 1 ] || return 0
  local parent; parent="$(cd .. && pwd)"
  if [ "$parent" = "$WORKSPACE_ROOT" ]; then
    echo "ci.sh: CI worktree sits inside $WORKSPACE_ROOT — ../host would" >&2
    echo "       resolve to the dev checkout instead of this sha. Move the" >&2
    echo "       worktree base outside the workspace root." >&2
    return 1
  fi
  ln -sfn "$PWD" "$parent/host"
  local d name
  for d in "$WORKSPACE_ROOT"/*/; do
    name="$(basename "$d")"
    [ "$name" = host ] && continue
    # refresh symlinks (the worktree host link moves every run); never
    # clobber a real directory that happens to sit in the parent.
    if [ -L "$parent/$name" ] || [ ! -e "$parent/$name" ]; then
      ln -sfn "$d" "$parent/$name"
    fi
  done
  # A symlinked ../host gives each self-patched crate a second path
  # spelling (<parent>/host/... vs <worktree>/...) — cargo treats them as
  # different packages and the lockfile write collides. The dagger module
  # avoided this by writing absolute paths in the self-patch; do the same
  # here: rewrite `path = "../host…"` to this worktree's absolute path.
  sed -i "s|\.\./host|$PWD|g" Cargo.toml
}

# Private git deps: same insteadOf trick as the dagger module, scoped to
# this process tree via GIT_CONFIG_* env — never touches ~/.gitconfig.
setup_auth() {
  local tok="${HANDL_TOKEN:-${BUPHA_TOKEN:-}}"
  [ -n "$tok" ] || return 0
  export GIT_CONFIG_COUNT=1
  export GIT_CONFIG_KEY_0="url.https://x-access-token:${tok}@github.com/.insteadOf"
  export GIT_CONFIG_VALUE_0="https://github.com/"
}

# sccache — optional shared remote backend; the env file is the same one
# the dagger runner mounted as a secret.
setup_sccache() {
  command -v sccache >/dev/null 2>&1 || return 0
  export RUSTC_WRAPPER=sccache
  if [ -f "$HOME/.config/bupha/sccache-env" ]; then
    set -a; . "$HOME/.config/bupha/sccache-env"; set +a
  fi
}

# ---------------------------------------------------------------------------
# Stages — 1:1 with the dagger module's functions.
# ---------------------------------------------------------------------------

stage_doctor() {
  rustc --version
  cargo nextest --version
  cargo llvm-cov --version
  sccache --version 2>/dev/null || echo "sccache: not installed"
  printf 'patch tables in Cargo.toml: %s\n' "$(grep -c 'patch\.' Cargo.toml || true)"
  echo "workdir:   $PWD"
  echo "worktree:  $IN_WORKTREE (main checkout: $MAIN_CHECKOUT)"
  echo "target:    $CARGO_TARGET_DIR"
  echo "siblings:  ../host-cli $( [ -d ../host-cli/crates/host-cli ] && echo present || echo missing )"
}

stage_test_shard() { # shard shards
  local shard="${1:-1}" total="${2:-$SHARDS}"
  cargo nextest run --workspace -j "$JOBS" --partition "count:${shard}/${total}"
}

stage_doctests() {
  # nextest does not cover doctests — the separate check.
  cargo test --doc --workspace -j "$BUILD_JOBS" -- --test-threads="$JOBS"
}

stage_test() {
  # Warm the cargo caches serially before any fan-out — the dagger module
  # did the same before spawning shard containers.
  cargo fetch --locked
  local i
  for i in $(seq 1 "$SHARDS"); do
    stage_test_shard "$i" "$SHARDS"
  done
  stage_doctests
  echo "test: $SHARDS shard(s) + doctests passed"
}

stage_coverage_report() {
  ./scripts/coverage.sh --summary
  cargo llvm-cov --workspace --lcov --output-path coverage/lcov.info
}

stage_coverage_gate() {
  # >=99% lines on the tooling capability crates (floor + rationale in
  # the script).
  ./scripts/coverage.sh --tooling-gate
}

# ADR-0149: `hnl` is a dumb host — `fmt`/`lint`/`typecheck`/`test` are
# `libhnl_ext_*` cdylibs built from the sibling ext-* repos and discovered
# at runtime via $HANDL_EXT_DIR. The gate must therefore build the ext
# libs too, and — on a detached worktree — against the sha under test.
DOGFOOD_EXTS=(ext-fmt ext-lint ext-typecheck ext-test)

stage_dogfood() {
  local cli_real="$WORKSPACE_ROOT/host-cli" scratch="" rc=0
  if [ ! -d "$cli_real/crates/host-cli" ]; then
    echo "dogfood: sibling checkout $cli_real not found — skipping"
    return 0
  fi
  local cli_dir="$cli_real" e d name
  local ext_dirs=() built_wts=()
  for e in "${DOGFOOD_EXTS[@]}"; do
    [ -d "$WORKSPACE_ROOT/$e" ] && ext_dirs+=("$WORKSPACE_ROOT/$e")
  done
  if [ "$IN_WORKTREE" = 1 ]; then
    # The repos being built must resolve `../host` to THIS sha — a plain
    # symlink won't do (cargo canonicalizes through it to the dev
    # checkout's parent). Give them a scratch parent: real git worktrees
    # for the repos we build, the sha as `host`, symlinks for the rest
    # (the dagger module did this with a mounted cli source + dev-link).
    scratch="$(mktemp -d "${TMPDIR:-/tmp}/hnl-ci-dogfood.XXXXXX")"
    ln -sfn "$PWD" "$scratch/host"
    for e in host-cli "${DOGFOOD_EXTS[@]}"; do
      [ -d "$WORKSPACE_ROOT/$e" ] || continue
      git -C "$WORKSPACE_ROOT/$e" worktree add --detach "$scratch/$e" >/dev/null \
        && built_wts+=("$e")
    done
    for d in "$WORKSPACE_ROOT"/*/; do
      name="$(basename "$d")"
      [ -e "$scratch/$name" ] || ln -s "$d" "$scratch/$name"
    done
    cli_dir="$scratch/host-cli"
    ext_dirs=()
    for e in "${built_wts[@]}"; do
      [ "$e" != host-cli ] && ext_dirs+=("$scratch/$e")
    done
  fi
  local tgt="$WORKSPACE_ROOT/.ci-target/dogfood"
  # Ext cdylibs first (into the shared dogfood target), then the gate
  # script builds hnl into the same dir and runs the corpus.
  for d in "${ext_dirs[@]}"; do
    echo "== build $(basename "$d") (libhnl_ext_*.so)"
    (cd "$d" && CARGO_TARGET_DIR="$tgt" cargo build --release) || {
      echo "dogfood: ext build failed in $d"; rc=1; break; }
  done
  if [ "$rc" -eq 0 ]; then
    HANDL_EXT_DIR="$tgt/release" HOST_CLI_DIR="$cli_dir" CARGO_TARGET_DIR="$tgt" \
      HNL_DOGFOOD_JOBS="$DOGFOOD_JOBS" bash scripts/dogfood-gate.sh || rc=$?
  fi
  if [ -n "$scratch" ]; then
    for e in "${built_wts[@]}"; do
      git -C "$WORKSPACE_ROOT/$e" worktree remove --force "$scratch/$e" 2>/dev/null
    done
    rm -rf "$scratch"
  fi
  return "$rc"
}

# The whole gate — serial. The dagger module could fan stages out across
# containers; on one 11 GiB box `ci --sequential` was already the honest
# mode, and it is the only mode here. All stages run so a failure report
# covers everything, same as the python version.
stage_ci() {
  local failures=() s
  for s in test coverage-gate dogfood; do
    if [ "$s" = dogfood ] && [ "${CI_SKIP_DOGFOOD:-0}" = 1 ]; then
      continue
    fi
    echo
    echo "=== ci: $s ==="
    "stage_${s//-/_}" || { failures+=("$s"); echo "=== ci: $s FAILED ==="; }
  done
  if ((${#failures[@]})); then
    echo "ci failed: ${failures[*]}" >&2
    return 1
  fi
  echo "ci passed: test, coverage-gate, dogfood"
}

# ---------------------------------------------------------------------------

stage="ci"
if [ $# -gt 0 ]; then stage="$1"; shift; fi

setup_auth
ensure_siblings || exit 1   # wrong layout = wrong sha under test; refuse
setup_sccache

case "$stage" in
  doctor)          stage_doctor ;;
  test)            stage_test ;;
  test-shard)      stage_test_shard "$@" ;;
  doctests)        stage_doctests ;;
  coverage-report) stage_coverage_report ;;
  coverage-gate)   stage_coverage_gate ;;
  dogfood)         stage_dogfood ;;
  ci)              stage_ci ;;
  -h|--help|help)  sed -n '2,31p' "$0" ;;
  *) echo "unknown stage: $stage" >&2; exit 2 ;;
esac
