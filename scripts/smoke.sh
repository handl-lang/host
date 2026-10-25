#!/usr/bin/env bash
# smoke.sh — hnl-level smoke suite over tests/smoke/.
#
# Drives the real `hnl` binary plus its libhnl_ext_* cdylibs — end-to-end
# coverage of the check/run/test surfaces no Rust suite exercises:
# data/match on indexed families, F.rec, Path/J/transport, IO<sig> +
# handle, instance + implicit evidence, Sigma pair-intro, type.* aliases,
# forall, ~[n]~> cells, export + `hnl run`/`--name`, eval.checked and
# sym.seeded mints — plus the expected-failure files.
#
# File conventions inside --dir (default tests/smoke):
#
#   <name>.hnl      positive — `hnl typecheck` must exit 0.
#   <name>_err.hnl  negative — `hnl typecheck` must exit nonzero AND its
#                   combined output must contain the fixed string in the
#                   required <name>_err.err sidecar (expected-error
#                   assertion).
#   <name>.out      `hnl run <name>.hnl` stdout must equal this file
#                   exactly (trailing whitespace normalized; stderr is
#                   diagnostics — e.g. the expected `lower_ast … falling
#                   back` note for self-recursive binds — not compared).
#   <name>.run      TSV of `--name` probes: each line
#                   `<expr>\t<expected>` runs `hnl run --name '<expr>'`
#                   and compares trimmed stdout.
#   <name>.test     marker file — `hnl test <name>.hnl` must exit 0.
#
# Adding a check = dropping files in; nothing here is enumerated.
#
#   scripts/smoke.sh [--hnl PATH] [--dir DIR] [-v|--verbose]
#
# Env:
#   HNL            hnl binary (default ~/.cargo/shared-target/debug/hnl —
#                  the dev toolchain from ../host-cli + ../ext-*; CI passes
#                  the release build it just produced).
#   HANDL_EXT_DIR  ext cdylib dir. Unset = hnl's dev-build discovery (its
#                  own dir and <dir>/ext); CI exports its dogfood target's
#                  release dir.
#   SMOKE_TIMEOUT  per-invocation timeout in seconds (default 120).
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

HNL="${HNL:-$HOME/.cargo/shared-target/debug/hnl}"
DIR="$REPO_ROOT/tests/smoke"
VERBOSE=0

while [ $# -gt 0 ]; do
  case "$1" in
    --hnl) HNL="$2"; shift 2 ;;
    --dir) DIR="$2"; shift 2 ;;
    -v|--verbose) VERBOSE=1; shift ;;
    -h|--help) sed -n '2,41p' "$0"; exit 0 ;;
    *) echo "smoke.sh: unknown arg: $1" >&2; exit 2 ;;
  esac
done

[ -x "$HNL" ] || { echo "smoke.sh: hnl not executable: $HNL" >&2; exit 2; }
[ -d "$DIR" ] || { echo "smoke.sh: suite dir missing: $DIR" >&2; exit 2; }

cd "$REPO_ROOT"
DIR="$(cd "$DIR" && pwd)"
RELDIR="${DIR#$REPO_ROOT/}"

PASS=0; FAIL=0; FAILED=()
ERRLOG="$(mktemp -t hnl-smoke-err.XXXXXX)"
trap 'rm -f "$ERRLOG"' EXIT

report() { # status label detail
  if [ "$1" = PASS ]; then
    PASS=$((PASS + 1))
    [ "$VERBOSE" = 1 ] && printf '  PASS  %s\n' "$2"
  else
    FAIL=$((FAIL + 1)); FAILED+=("$2")
    printf '  FAIL  %s  — %s\n' "$2" "$3"
  fi
}

trim() { sed -e 's/[[:space:]]*$//' -e '/^$/d' ; }
# ^ normalizer for compared output: strip trailing ws + blank lines. The
# .err/.out goldens are written the same way, so comparison is on
# content, not formatting tail.

# hnl invocation with stderr captured to $ERRLOG; stdout on stdout.
# For typecheck checks the caller merges them (expected errors can land
# on either stream); for run goldens only stdout is compared.
run_hnl() { timeout "${SMOKE_TIMEOUT:-120}" "$HNL" "$@" 2>"$ERRLOG"; }

for f in "$DIR"/*.hnl; do
  base="$(basename "$f" .hnl)"
  rel="${f#$REPO_ROOT/}"
  case "$base" in
    *_err)
      # Expected failure: typecheck must exit nonzero AND carry the
      # expected-error string from <base>.err.
      err_sidecar="$DIR/$base.err"
      out="$(run_hnl typecheck "$rel")"; rc=$?; out="$out$(cat "$ERRLOG")"
      if [ $rc -eq 0 ]; then
        report FAIL "$base: typecheck" "expected nonzero exit, got 0"
      elif [ ! -f "$err_sidecar" ]; then
        report FAIL "$base: typecheck-fail" "missing expected-error sidecar $base.err"
      elif ! printf '%s' "$out" | grep -qF "$(trim < "$err_sidecar")"; then
        report FAIL "$base: typecheck-fail" \
          "output lacked '$(trim < "$err_sidecar")' — got: $(printf '%s' "$out" | head -1)"
      else
        report PASS "$base: typecheck-fail" ""
      fi
      ;;
    *)
      out="$(run_hnl typecheck "$rel")"; rc=$?; out="$out$(cat "$ERRLOG")"
      if [ $rc -eq 0 ]; then
        report PASS "$base: typecheck" ""
      else
        report FAIL "$base: typecheck" "exit $rc — $(printf '%s' "$out" | head -1)"
      fi
      ;;
  esac
done

# .out — `hnl run` golden stdout (stderr diagnostics not compared).
for o in "$DIR"/*.out; do
  [ -e "$o" ] || continue
  base="$(basename "$o" .out)"
  rel="$RELDIR/$base.hnl"
  [ -f "$DIR/$base.hnl" ] || { report FAIL "$base: run" "missing $base.hnl for .out"; continue; }
  out="$(run_hnl run "$rel")"; rc=$?
  got="$(printf '%s' "$out" | trim)"
  want="$(trim < "$o")"
  if [ $rc -ne 0 ]; then
    report FAIL "$base: run" "exit $rc — $(printf '%s' "$out"; cat "$ERRLOG" | head -1)"
  elif [ "$got" != "$want" ]; then
    report FAIL "$base: run" "stdout vs $base.out — $(diff <(printf '%s\n' "$want") <(printf '%s\n' "$got") | head -4 | tr '\n' ' ')"
  else
    report PASS "$base: run" ""
  fi
done

# .run — TSV of `hnl run --name <expr>` probes.
for r in "$DIR"/*.run; do
  [ -e "$r" ] || continue
  base="$(basename "$r" .run)"
  rel="$RELDIR/$base.hnl"
  n=0
  while IFS=$'\t' read -r expr expected; do
    case "$expr" in ''|'#'*) continue ;; esac
    n=$((n + 1))
    out="$(run_hnl run --name "$expr" "$rel")"; rc=$?
    got="$(printf '%s' "$out" | trim)"
    if [ $rc -ne 0 ]; then
      report FAIL "$base: --name $expr" "exit $rc — $(printf '%s' "$out"; cat "$ERRLOG" | head -1)"
    elif [ "$got" != "$expected" ]; then
      report FAIL "$base: --name $expr" "want '$expected', got '$got'"
    else
      report PASS "$base: --name $expr" ""
    fi
  done < "$r"
  [ "$n" -eq 0 ] && report FAIL "$base: --name" "$base.run has no probes"
done

# .test — `hnl test` marker.
for t in "$DIR"/*.test; do
  [ -e "$t" ] || continue
  base="$(basename "$t" .test)"
  rel="$RELDIR/$base.hnl"
  out="$(run_hnl test "$rel")"; rc=$?; out="$out$(cat "$ERRLOG")"
  if [ $rc -eq 0 ]; then
    report PASS "$base: hnl test" ""
  else
    report FAIL "$base: hnl test" "exit $rc — $(printf '%s' "$out" | tail -1)"
  fi
done

echo
echo "smoke: $PASS passed, $FAIL failed"
if [ "$FAIL" -gt 0 ]; then
  printf '  failed: %s\n' "${FAILED[@]}"
  exit 1
fi
