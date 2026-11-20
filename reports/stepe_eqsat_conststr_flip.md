---
title: Step E flip #1 — eqsat.py const-str → canonical arr leaf profile
type: report
date: 2026-11-17
status: GO — landed on branch stepe-eqsat-arr-leaf @ 595bb35 (pushed; main merge + v0.1.13 tag are parent's call)
repo: /home/luis/GitHub/handl-lang/eqsat
census: stepe_dialect_census.md §7 item 1
---

# Step E flip #1 — eqsat bridge emits canonical `Arr` leaves

## Verdict: GO

The three transitional `leaf="const-str"` emit sites flipped to
`leaf="arr"`, all gates green, live `eqsat-run` e2e and the WP4
parity sweep pass end-to-end on the Arr wire.

## Change

`packages/mlopt-orchestrator/src/mlopt_orchestrator/eqsat.py` —
one commit, `595bb35` on branch `stepe-eqsat-arr-leaf` (based on
`d67d1fc` / v0.1.12, pushed to origin):

- `_kterm` (was :392) — `term_to_json(dag="tree", leaf="arr")`
- `_kdag` (was :402) — `term_to_json(dag=True, leaf="arr")`
- `_terms_wire` (was :417) — `terms_to_json(leaf="arr")`
- Module docstring leaf-spelling paragraph + the codec-delegation
  comment block updated to describe the canonical `Arr` profile and
  the `dd4815e` engine fix that unblocked it.

Decode is untouched: `term_json.term_from_json` accepts both leaf
spellings permanently (tree and dag positions), so records emitted
by older binaries / retained fixtures still ingest.

## Decode-side / consumer audit

- `_pyterm` delegates to `term_json.term_from_json(validate=False)`
  with no leaf-profile argument — Arr and const-str both decode;
  no eqsat.py code path assumes the const-str spelling on ingest
  (grepped `__mlopt_leaf__` handling: only decode tests pin it).
- `Var.typ`/`Param.typ` are typed `TensorType` in
  `mlopt_core.term` — the untyped-`Var` emit need that forced
  const-str in graphopt's `bench/drivers/audit/eqsat_differential.py`
  (`__var__:`/`__var__:param:` fallback for `typ="f64"` string
  fixtures) **cannot arise here**: that fixture shape lives outside
  this repo's term contract. No STOP condition.
- `graphopt`'s deliberate const-str exceptions are untouched —
  different repo, different wire (its own `_kterm`/`_pyterm` codec).

## Test results

- `pytest tests/test_eqsat_engine.py` — **27 passed, 0 skipped**
  (live `requires_eqsat` tests ran against
  `engine-eqsat/target/release/eqsat-run`, the Arr-capable binary).
- `pytest tests/ -q -m "not slow"` — **1885 passed, 15 skipped,
  2 deselected** in 17.2 s. (The two kit-eqsat sibling-drift
  failures the dry-run saw are gone — sibling checkout has since
  converged.)
- `ruff check packages` — clean. `ruff format --check packages` —
  clean for touched files; one **pre-existing** unformatted file
  (`mlopt-core/src/mlopt_core/ops.py:350`) on main, untouched.
- `ty check` — 0 errors. `lint-imports` — 3/3 contracts kept.
- Manual wire probe: `_terms_wire([Op.make("neg", Var("x",
  TensorType((2,3))))])` now emits
  `{"Arr": {"dim": {"Const": {"value": 1}}, "value":
  {"__mlopt_leaf__": "var", "name": "x", "shape": [2, 3]}}}` and
  `_pyterm` round-trips it `==`.

## Parity sweep (graphopt bench driver, eqsat packages overlaid)

`bench/drivers/audit/eqsat_parity_sweep.py` (9 default cases ×
python/eqsat arms, `PYTHONPATH` overlay of this branch's
`mlopt-orchestrator`/`mlopt-core`/`mlopt-dtypes` src dirs,
`MLOPT_EQSAT_RUN` → release binary): **full parity** — identical
extracted costs on every case, `verified=True` on every arm,
`max_abs` identical between engines. Result JSON:
`/tmp/stepe_parity_arr.json`. This exercises the real
saturation → extract → verify path through Arr leaves, not just
the codec round-trip.

## Notes for the parent

- Branch `stepe-eqsat-arr-leaf` is pushed; **not merged** — eqsat
  tags are consumer-facing (graphopt/kernelopt/mlopt pin `v0.1.10`,
  kit-* pin `v0.1.12`), so the `v0.1.13` tag decision and consumer
  repins are yours.
- Remaining const-str emitters per census: only
  `graphopt/bench/drivers/audit/eqsat_differential.py`'s private
  codec (deliberate untyped-Var exception) and the codec's own
  `const-str` profile arm (permanent decode + profile machinery).
- `eqsat_differential.py` was not re-run — it uses its own codec,
  not `mlopt_orchestrator.eqsat`, so this flip cannot affect it.
