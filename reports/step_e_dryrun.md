---
title: Step E dry-run — plan B (term repr convergence) feasibility, E-2a + E-3 codec delegation
type: report
date: 2026-11-17
status: dry-run complete — feasibility validated, worktree left at /home/luis/tmp/eqsat-step-e (detached @ 4eae588, uncommitted)
scope: eqsat only — packages/mlopt-core/{term.py,term_json.py,ir.py} + packages/mlopt-orchestrator/eqsat.py + tests/test_term_json.py; kit_discovery.hnl_codec shim NOT exercised (different repo)
---

# Step E dry-run — Option B feasibility

Worktree: `/home/luis/tmp/eqsat-step-e` (`git worktree add … origin/main
--detach`, `uv sync --group dev` inside it — its `.venv` binds the
worktree's own packages, verified by `__file__` probe). Nothing pushed;
the real repos are untouched.

## What was built

| file | state | LOC |
|---|---|---|
| `packages/mlopt-core/src/mlopt_core/term.py` | NEW — ir.py's class half moved verbatim (TensorType/Var/Const/Param/Op+make/_INTERN/validate_attrs, IR+roots, GenDef/op_def/generator/_OP_REGISTRY + all `op_def(...)` calls, op_repr/op_repr_dag/_tree_sizes/_dag_*/_sexp, attr_to_data/attr_from_data) | 757 |
| `packages/mlopt-core/src/mlopt_core/term_json.py` | NEW — the ONE kernel-Term-JSON codec: hnl_codec generalized + `_kdag`'s dim-index node table + `wire_node_budget` cell + leaf profiles + `validate` decode flag | 682 |
| `packages/mlopt-core/src/mlopt_core/ir.py` | shim — explicit re-exports of term.py (public names via `__all__`, private helpers via `X as X` re-export lines) + the lemma-store codec kept (`term_to_data`/`term_from_data`/`_DAG_TAG`/`_leaf_*`/`_term_*_dag`) | 956 → 265 |
| `packages/mlopt-orchestrator/src/mlopt_orchestrator/eqsat.py` | codec quartet (~330 ln) replaced by 4 thin delegates over `term_json` (`_kterm`/`_kdag`/`_terms_wire`/`_pyterm` kept as wrappers — the test-suite pins them); helpers/constants deleted; imports cut to `Op` + `term_json` | 1106 → 824 (−282) |
| `tests/test_term_json.py` | NEW — port of `kit-discovery/tests/test_hnl_codec.py` (462 ln) + divergence/budget/terms_to_json coverage | 695 |
| `tests/test_ir.py` | 1-line fix — the `_OP_REPR_BUDGET` monkeypatch now targets `mlopt_core.term` | +1/−1 |

## Divergences actually hit (all three confirmed, plus four the plan didn't name)

1. **Leaf spelling.** Canonical `{"Arr": {"dim":…, "value": {"__mlopt_leaf__": "var", "name": "x", "shape": [2,3]}}}` vs legacy `{"Const": {"value": "__mlopt_leaf__:{\"k\":\"var\",\"n\":\"x\",\"s\":[2,3]}"}}`. Codec emits `leaf="arr"` (canonical) or `leaf="const-str"` (eqsat profile); decode accepts both permanently, in tree AND dag positions.
2. **DAG `Op.dim`.** Emitted as a node-table *index* (engine-eqsat `dag.rs` schema — a `Const(1)` node emitted once per record, shared); decode additionally accepts the legacy nested `{"Const":{"value":1}}` record and ignores it. I made dim-index *validation* stricter than either predecessor (both ignored `dim` entirely): an int dim must be an earlier-node index.
3. **`source`.** Emitted `"Native"` always (law patterns pin `source:"native"`, exact-keyset). Decode ignores `source` entirely — verified against `"Native"`, `{"Foreign":"tensor"}`, `{"Foreign":"onnx"}`, `"Judgment"`, `{"DynamicForeign":"x"}`, `null` — all decode identically.

New divergences/gotchas the dry-run surfaced:

4. **`_kterm` is NOT `dag=False`.** The old tree walker *expanded* sharing (budget-guarded); `dag=False` *refuses* shared terms (hnl_codec's contract). Needed a third mode: `term_to_json(…, dag="tree")` — force tree expansion, always budget-counted. Without it `test_codec_roundtrip` fails.
5. **`validate=False` decode.** `_pyterm`'s `_op_from_node` used `Op.make(…, validate=False)`; hnl_codec used the `True` default. `term_from_json` takes `validate: bool = True`; the eqsat wrapper passes `False`.
6. **`_has_sharing`/dag dedup keyed on content, not `id()`.** Plan-endorsed improvement, but it changes `dag=False` semantics: equal-but-distinct leaves (`Var`/`Param`/`Const` are NOT interned — only `Op` is) now count as "shared", so `dag=False` refuses slightly more terms and `dag=None` auto-selects the record more often. Decode is unaffected (merged nodes are equal). Also merges structurally-equal non-interned subtrees in the node table — strictly smaller records.
7. **Monkeypatched module privates break.** `test_ir.py` patched `mlopt_core.ir._OP_REPR_BUDGET`; `op_repr` now reads `mlopt_core.term._OP_REPR_BUDGET`. One test updated. Expect a handful of similar pins repo-wide at real E-2a (census: `headshare._attr_key`, `features._dag_postorder`, `vocabdir`/`torch_bindings._OP_REGISTRY` are *imports*, which the shim covers — only `setattr` sites break).
8. **Ruff F811 caught a splice bug**: my first ir.py kept verbatim `attr_to_data`/`attr_from_data` defs AND imported them — the lemma codec now uses term.py's (identical code). Shim surface verified: all 27 historical `mlopt_core.ir` attributes (public + private + `_DAG_TAG`/`_TERM_DATA_DAG_BUDGET`) still resolve.

## dd4815e verified live — the leaf-profile question is closer than the plan thought

The task said the engine-eqsat `arr`-enode fix (dd4815e) "already landed". Verified against the real binaries:

- `target/debug/eqsat-run` (post-dd4815e): canonical `leaf="arr"` dag record → `ok:true`, returned term decodes `==` the input (Var+Param+Const leaves all correct).
- `target/release/eqsat-run` (stale, pre-dd4815e): same record → `ok:false, "bad term: dag output: enode attr \"value\" has no kernel Term spelling"` — the exact `dag.rs:411` failure the plan predicted. `leaf="const-str"` works on BOTH.

So: the `const-str` profile is still required *today* (release binary stale), but the canonical `Arr` spelling is proven end-to-end on engine-eqsat main — E-3 could emit `leaf="arr"` as soon as the binary is rebuilt, and the profile parameter may not need to survive long.

## Test results

- `tests/test_term_json.py` + `test_ir.py` + `test_property_ir.py` + `test_eqsat_engine.py`: **145 passed** (incl. the real-eqsat-run e2e tests — binary resolved, `const-str` wire exercised through actual saturation).
- Full fast lane `pytest tests/ -q -m "not slow"`: **1883 passed, 2 failed, 15 skipped** — the 2 failures (`test_eqsat_laws_corpus.py::test_shipped_snapshot_matches_kit_corpus`, `test_law_bank_corpus_parity.py::test_non_prim_record_fields_match_except_named`) are **pre-existing on clean HEAD** (verified via stash): local `kit-eqsat` sibling checkout drift, unrelated.
- `ruff check` / `ruff format --check` on touched files: clean (one pre-existing unformatted `ops.py` on main, untouched).
- `ty check`: 0 errors. `lint-imports`: 3/3 contracts kept.

## LOC delta

Shipped source: ir.py −691 (moved), eqsat.py −282, +term.py 757, +term_json.py 682 → net **+466** lines in eqsat, but the codec consolidation itself is −507 vs the two predecessors it replaces (hnl_codec 479 + eqsat quartet ~330 = ~809 → term_json 682, which also absorbs budget/dag="tree"/validate/leaf-profile machinery neither had). Tests: +695 (ported suite).

## What breaks / open items for real E-2a/E-3

- `kit_discovery.hnl_codec` → shim is unexercised here (other repo). Its consumers (`mlopt` adapters, `stage.py`, emit path) need a kit-discovery worktree pass; signature is a straight re-export (`HnlCodecError`→`TermJsonError` alias).
- **Emit-side change**: canonical codec emits `source:"Native"` + `attrs` field where hnl_codec emitted `{"Foreign":"tensor"}` + `lit.__mlopt_attrs__`. Decode is tolerant both ways, but any consumer reading `term_to_json` *output JSON directly* (not through `term_from_json`) sees different spellings — audit needed (source-onnx docs, stage payloads, fixtures).
- `stage.py` RECORD_VERSION: plan recommends v4 bump; kit-prelude `RECORD_SCHEMA_VERSION` repin in same window.
- eqsat.py kept `_kterm`/`_kdag`/`_pyterm`/`_terms_wire` as wrappers because tests pin the private names — real E-3 should either update `test_eqsat_engine.py` to the public codec or keep the wrappers deliberately.
- `_kdag`/`_pyterm_dag`/`_op_from_node`/`_leaf_from_payload`/`_leaf_payload`/`_shape_of`/`_wire_value`/`_unwire_value` and the `_LEAF_PREFIX`/`_NONFINITE_*`/`_DIM1`/`_DAG_TAG` constants are deleted from eqsat.py; grep confirms no in-repo users remain.

## Go/no-go

**GO for ratifying B.** The move is mechanical and the codec unification is real: one module now speaks both predecessor wire dialects (encode canonical, decode everything), the dep-graph constraint is satisfied (codec in mlopt-core, orchestrator reaches it with zero new edges), the eqsat bridge delegates with a −282-line diff and all tests green including live `eqsat-run` e2e, and the risky leaf-spelling question resolved *better* than planned — the Arr fix is already proven on engine-eqsat main. Caveats to carry into ratification: (a) B remains nominal-Term-native per plan R1 — state that honestly; (b) the `source:"Native"`/`attrs`-field emit change needs a consumer audit before E-4 flips adapters; (c) kit-discovery's shim + stage RECORD_VERSION are the remaining unproven seams.
