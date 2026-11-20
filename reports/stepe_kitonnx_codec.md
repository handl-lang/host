---
title: Step E tail — kit-onnx Term-JSON codec migration dry-run
type: report
date: 2026-11-17
status: complete — branch committed, not pushed (report-first per task)
scope: kit-onnx codec-site census, mlopt_core.ir → mlopt_core.term repoint, parity vs source-onnx eb87b63
---

# Step E tail — kit-onnx codec migration

Repo: `/home/luis/GitHub/handl-lang/kit-onnx` (was clean on `main`
@ `084897d`, eqsat already pinned `v0.1.12` / `d67d1fc` in
`pyproject.toml:35-36`). Worked directly on branch
**`stepe/term-json-repoint`**, commit **`b2c8ff6`** — not pushed.

## 1. Codec-site census — the premise needed correcting

The task brief expected `lift.py`/`lower.py`/`source.py` to "do
Term-JSON codec work." **They do not.** A full-repo grep
(`json|serialize|term_to|term_from|to_data|from_data` over
`src/` + `tests/`) finds **zero** Term-JSON codec sites:

- `lift.py` / `source.py` — ONNX → view `IR` (`OnnxSource`,
  `lift_model`); objects, never JSON.
- `sink.py` / `lower.py` — view `IR` → ONNX graph; objects in,
  ONNX protobuf out.
- `opmap.py` / `liftmap.py` / `arrays.py` / `executor.py` /
  `report.py` — codec *tables* (IR↔ONNX op vocabulary) and the
  onnxruntime runner; no term serialization.

This matches `stepe_tail_plan.md` §3's matrix row: *"kit_onnx
lift/source/lower/sink — view-object producer/consumer — n/a (no
JSON) — repoint imports only"* and §7's "kit-onnx import repoint —
S (6 files)".

**Consequence for the parity-oracle role:** kit_onnx never emits
Term-JSON itself. In `graphopt/bench/drivers/audit/
onnx_lifter_differential.py` the Python side lifts via
`kit_onnx.lift.lift_model` to a view term, then **`mlopt_core.
term_json.term_to_json`** (eqsat, canonical: `Arr` leaves,
`"Native"` source, `attrs` channel, dag dim-index) does the wire
encode; the Rust side is `source-onnx`'s `lift` example binary.
So kit_onnx's only influence on wire spelling is *which view-term
objects it constructs* — and the repoint cannot change those:
`mlopt_core.term` and the `mlopt_core.ir` shim re-export the
**identical class objects** (verified: `ir.IR is term.IR`, etc.,
for all six names).

## 2. Migration performed

`from mlopt_core.ir import …` → `from mlopt_core.term import …`
in all six src files (the plan's class-a construction sites
`lift.py:56`, `liftmap.py:58`, `lower.py:47` plus class-b
`sink.py:24`, `source.py:36`, `opmap.py:56`), plus two docstring
references (`lift.py:4`, `lower.py:3`). One ruff isort fix in
`sink.py` (import reordering). `tests/test_onnx_opmap.py:22`
deliberately stays on `mlopt_core.ir` — the Step-E convention is
that test imports ride the shim.

No `term_json` import was added — there is no codec call site to
route through it. The `leaf=`/`dag=`/`source`/`attrs`/`dim`
profile questions are moot for this repo; the emit profile is
owned by `term_json` defaults (`leaf="arr"`, canonical) and by
`source-onnx` `term.rs`, both already canonical.

## 3. Verification

kit-onnx gates (branch, `.venv` synced to eqsat `d67d1fc`):

- `pytest tests/ -q` — **41 passed**
- `ruff check src` / `ruff format --check src` — clean
- `ty check` — clean
- `lint-imports` — leaf contract kept (1 contract, 0 broken)
- `coverage run -m pytest` — **43%** = `fail_under` floor (exit 0)

Parity vs source-onnx **`eb87b63`** (canonical emit: `"Native"`
source + dag dim-index; `cargo build --example lift` re-run
clean at that HEAD):

- `graphopt-stepe` worktree's migrated
  `bench/drivers/audit/onnx_lifter_differential.py`, run with
  `PYTHONPATH=<kit-onnx>/src` so the **branch** kit_onnx is the
  oracle (`lift` binary = source-onnx `target/debug/examples/
  lift` @ eb87b63):
  - full corpus `--no-eval`: **17 models — 10 parity,
    7 refuse-parity, 0 divergences** (incl.
    `whisper_tiny_encoder` term=iso 219/241 nodes, resnet +
    12 fixtures)
  - fixtures with `--eval`: **9 parity+eval:equal,
    6 refuse-parity, 0 divergences**
  - reports: `/home/luis/tmp/kitonnx_diff{,_eval}.json`
- Cross-stack suite in the same env
  (`PYTHONPATH` override): `tests/test_onnx_codec_drift.py` +
  `test_onnx_liftmap.py` + `test_onnx_sink.py` +
  `test_onnx_source.py` — **370 passed**.

Parity verdict: **green.** kit_onnx's lifted view terms encode
through canonical `term_json` to records isomorphic to
source-onnx eb87b63's emit on every corpus model; named refusals
match family/op on all refuse fixtures.

## 4. Go / no-go

**Go** — with two notes for the parent agent:

1. **Branch `stepe/term-json-repoint` @ `b2c8ff6` is committed
   locally, NOT pushed** (per "report first"). It is a 6-file,
   8-line repoint; safe to push and tag (`v0.1.4`?) whenever the
   Step-E cascade wants it — graphopt's kit-onnx pin is still
   `v0.1.3` and would need a repin to consume it, but nothing
   requires that urgently since the change is behavior-identical.
2. **`mlopt_core.ir` census:** kit-onnx src is now at **zero**
   `mlopt_core.ir` imports; `tests/test_onnx_opmap.py:22`
   remains on the shim per convention. This closes kit-onnx's row
   in the Step-E ir→term census (plan §7).
