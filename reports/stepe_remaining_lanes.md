# Step E tail sweep — remaining lanes

Date: 2026-11-16 (post eqsat d67d1fc, `mlopt_core.term` + `term_json` landed, `ir.py` is a compat shim).

## Verdicts

### 1. eqsat `packages/mlopt-native/python/mlopt_native/engine.py:51` — NO-OP (honest)

- Sole import: `from mlopt_core.ir import Const, Op, Param, Var` (line 51). These are the
  term *types*; `ir.py` re-exports them from `mlopt_core.term`.
- The module does **no codec work**: `_flatten` / `_ser_pattern` (lines 62+) serialize
  terms/patterns to plain tuples for the PyO3 FFI wire (`("op", op, idxs, attrs)` nodes) —
  that is the Rust binding's private format, not the `term_json` envelope. Nothing here
  should move to `term_json`.
- Repointing `ir` → `term` would be cosmetic, and it would go **against the repo's own
  convention**: every other `mlopt_core` module still imports through the `ir` shim
  (`features.py:22`, `meta.py:107`, `oracle.py:38`, `egraph/extract.py:27`, … — ~20 sites).
  The only `mlopt_core.term` import in the repo is the *uncommitted WIP* on branch
  `stepe-eqsat-arr-leaf` (`mlopt_orchestrator/eqsat.py:120`, dirty tree).
- PyO3 note (as predicted): `Const/Op/Param/Var` are the same classes via the shim —
  no Rust-binding impact either way.
- **Caveat for parent:** eqsat working tree is NOT clean — branch `stepe-eqsat-arr-leaf`
  has `M packages/mlopt-orchestrator/src/mlopt_orchestrator/eqsat.py`. I made no changes.

### 2. gamekit + benchkit — ZERO sites, confirmed

- `gamekit/packages/gamekit/src/gamekit/players/neural/__init__.py:81-82` — lazy imports
  are `mlopt_core.features.compute_features` and `mlopt_core.trajectories.{RuleSample,
  rule_vector}` only (the sanctioned lazy domain seam, `ty: ignore`'d, absent-by-design).
- `gamekit/packages/gamekit/src/gamekit/engine.py:4` — docstring mention of
  `mlopt_core.search_env.SearchEnv` only.
- `gamekit/packages/benchkit/src/benchkit/runner.py:18` — `from mlopt_core.timing import
  iqr, median` (timing stats, not codec).
- No `mlopt_core.ir`, `mlopt_core.term`, or `term_json` anywhere in shipped source or tests
  (test_folder_seam.py stubs `mlopt_core.features`/`trajectories` modules only).

### 3. eqsat `packages/mlopt-dtypes` — ZERO

- `src/mlopt_dtypes/__init__.py` mentions `mlopt_core` once, in a docstring explaining why
  the registry is *not* in mlopt_core. `pyproject.toml`: `dependencies = []` — pure stdlib
  leaf. No term/codec surface.

### 4. kit-prelude — ZERO Python codec

- Only `.py` file in the repo (outside `.venv`/`target`):
  `crates/kit-mlopt/tests/fixtures/stage_stub.py` — a test stub emitting
  `"format": "hnl"` JSON; no term codec.
- `crates/hnl-ext-mlopt` is the Rust cdylib shim that spawns `python -m mlopt`; the
  `.hnl` emit/eval machinery lives in the `mlopt` repo (`emit.py`, `hnl.py` adapter),
  already covered by earlier lanes.

## Changes made

None. All five lanes are honest no-ops; no files edited, no commits, no branches created.

## Follow-up for parent

- eqsat dirty tree (`stepe-eqsat-arr-leaf` branch, modified `eqsat.py`) is pre-existing
  WIP, not from this sweep — flag if it should have been clean.
