---
title: ADR-0174 exit-criteria audit — pass 3 (post-pass-2 wave)
type: report
date: 2026-11-15
---

# ADR-0174 exit-criteria audit — pass 3 (post-pass-2 wave)

Scope: verify the commits landed since pass 2 hold the
honesty/verification invariants, and hunt regressions in the
kit-eqsat → eqsat_laws → engine-eqsat → source-onnx → mlopt chain.
Pass-2 FIXED findings spot-checked (F2/F3/F7/F9/F10 surfaces re-read;
all still hold — see below). Uncommitted WIP in `engine-eqsat`
(~78-law batch: `laws/{batch_norm,conv_folds,prim_folds}.json` +
modified `src/{builtin,guard,shape}.rs`, `tests/contracts.rs`)
was present throughout and **not disturbed**; several observations
below are about that WIP's interaction with the drift guards, which
is the guard working as designed.

## Per-commit verdicts

### engine-eqsat

- **`b9fe227` (bridge leaf shape decode) — CLEAN, and it closes a real
  soundness hole.** `__mlopt_leaf__:` `Const` strings previously fell
  through to the scalar-`()` default, so broadcast guards saw phantom
  scalars and fired where real shapes would decline (the WP4
  `mul_unsq_pad` parity failure: extracted `mul(a, unsqueeze(b,-1))`
  could not materialise — `lower()` crashed *before* verify could
  judge). Now `shape::mlopt_leaf_info` decodes the payload's declared
  `TensorType` shape; `s: null`/malformed → `Shape::Unknown`, never
  scalar (`src/shape.rs:2106-2145`). Unknown is poisonous in the
  shape algebra, so guards decline — the honest direction. Regression
  tests pin veto + positive control (`tests/eqsat.rs`,
  `tests/shape.rs`).
- **`308443b` (bridge leaves not consts) — CLEAN.** `const_value`
  routes through `shape::is_bridge_leaf`: prefixed strings decline
  every const read (`const`, `const-num`, `const-cmp`, the
  `("const", T)` derive expr), matching cond.py's
  `isinstance(t, Const) == False` for `Var`/`Param`
  (`src/guard.rs:384-401`). Malformed payloads still classify as
  leaves — decline, never guess. `term-eq` unaffected (full-encoding
  compare). Test `bridge_leaf_is_not_a_const` is thorough.
- **`90a0058` (guarded law batch, ~78 laws + `attrs_fields`) —
  CLEAN at HEAD.** `AttrsSpec::Fields` implements the Python
  `Op.make` per-key attr-metavariable contract honestly:
  `expand_attr_fields` (`src/law.rs`) requires exact keyset
  (`fields.len() != binding.fields.len()` → decline), literal pins
  must `match_eq`, repeated binds must agree; `synthesize_attr_payload`
  declines (`None`) on unbound RHS fields. `Law::compile` wires
  check-before-derive in `EGraph::apply_rule` order. Exclusions are
  documented per-pack in the commit message and module docs
  (`mul_unsq_nat` flat-map-unsq, `rms_norm_fold*`/`batch_norm_expand*`
  shape dspec, `conv_bn_fold`/`prim:synop*` procedural hooks, conv_*
  family, scale-free sdpa expands). **`reshape_flatten_demo` remains
  quarantined**: standalone `pub fn` at `src/builtin.rs:2994` (HEAD),
  absent from every pack fn, `tests/contracts.rs:622-625` pins it as
  the matcher-only demo, and it is excluded from kit-eqsat and the
  eqsat snapshot (asserted by `test_eqsat_laws_corpus.py:70`).
  `cargo test` on the working tree (incl. WIP): all green.

### eqsat

- **`a782f36` (snapshot → 111 records) — CLEAN.** Snapshot now the
  full non-demo corpus; verified: 111 records, 111 unique names,
  `reshape_flatten_demo` absent.
- **`631b889` (eqsat_laws.json GENERATED) — CLEAN; the drift chain
  is real.** `scripts/emit_eqsat_laws.py` reads `LAW_PACKS` order out
  of kit-eqsat's `lib.rs` (regex fail → loud `SystemExit`, not a
  guess), concatenates `data/laws/<pack>.json`, renders canonically
  (`json.dumps(indent=1, sort_keys=True) + "\n"`). `--check` does a
  **byte-exact** comparison and exits 1 on drift, 2 when kit-eqsat is
  absent — verified live: `--check` → "in sync", exit 0.
  `tests/test_eqsat_laws_corpus.py` asserts *both* semantic equality
  (`shipped == records`) and byte-exact rendering, plus `len == 111`
  and demo exclusion; it **passed** (12.5 s). The skip-when-absent
  path is honest (documented, mirrors kit-eqsat's own convention) —
  the only silent-divergence hole is a standalone clone without the
  sibling, which is the accepted convention.
- **`84f059c` merge — CLEAN.** The `stats["improved"]` null-guard
  survived (`eqsat.py` `run()` pops it when the CLI reports null).
- **`77542a2`/`e307a75` bridge (`eqsat.py`, 720 lines) — exemplary
  honesty.** Every non-portable surface is refused by name:
  `stop != "fixed_point"` → `ValueError`; `extract_best` refuses
  `overrides`/`bans`/`_cache_out`/`fusion_epsilon` by name;
  unportable `cost_fn` is *recorded* in `stats["eqsat"]`, not
  silently dropped; uncovered requested laws are listed verbatim in
  `stats["eqsat"]["laws"]["uncovered"]`; missing `eqsat-run` binary
  fails loudly at construction (`EqsatUnavailableError`), never
  mid-search, never by fallback. Module docstring correctly states
  the verify gate downstream is unaffected (e-class equivalence +
  `lower(verify=True)`).

### kit-eqsat `30238cf` — CLEAN, guard verified live.

- `tests/corpus_drift.rs` parses both sides to `serde_json::Value`
  and asserts pack-for-pack equality against
  `engine-eqsat/laws/<name>.json`; missing/unparseable sibling files
  panic loudly ("missing" — slightly misleading message on corrupt
  JSON, still a failure). Skips only when the sibling dir is absent.
  **Verified it actually fires**: with the uncommitted engine-eqsat
  WIP in tree, `cargo test` → `kit_corpus_matches_engine_eqsat_reference`
  FAILED with a real drift diff. The guard works.
- At HEAD the corpora are semantically identical (verified all five
  packs: `simplification`/`categorical`/`attention` byte-differ only
  in formatting — serde compact vs emit_laws pretty — `prior_art`/
  `pointwise_factor` byte-identical; parsed `Value`s equal).
- No phantom records: `record_names_unique` pins name uniqueness,
  `version == 2`, `kind == "law"`, lhs/rhs objects; `packs_parse_and_total_111`
  pins the count. `reshape_flatten_demo` asserted absent
  (`law_pack("reshape_flatten_demo").is_none()`).
- Data-only crate, no handl deps — the dependency arrow stays
  interpret/Python → kit data as documented.

### source-onnx `a2b28cb` — CLEAN; honest refusals verified.

- Spot-checked ported lifters: `select_last_index=1` ArgMax/ArgMin
  refuses with the *reason* (ATen picks first extremum —
  `lifter.rs:938-949`); asymmetric `pads` and any `auto_pad` value
  other than `"NOTSET"` refuse (`spatial_window`,
  `lifter.rs:995-1038`); strides/dilations rank mismatch refuses.
  Every refusal is `LifterError::Unsupported { op_type, node, reason }`
  — named, never a silent skip.
- Shape oracle (`shapes.rs`) is deliberately conservative: onnx-ir
  renames every value, so pairing is positional *only* when raw and
  typed bucket counts agree — fused/folded/remapped buckets contribute
  nothing; the untyped `ArgType` default (rank-0, `static_shape:
  None`) reads as absent, matching Python's `shape() -> None`.
  Inference failure degrades to an empty oracle → shape-dependent ops
  refuse by name. No fabricated shapes found.
- `cargo test`: all green including the two artifact tests
  (`whisper_tiny_encoder_lifts`, `light_resnet50_opset_refusal` —
  real artifacts, env-gated skip documented).
- **Coverage caveat (new finding P3-N1, LOW):** the "107/107
  ONNX_LIFTS" completeness claim has **no drift guard** — nothing
  ties the Rust lift table to `mlopt_onnx.liftmap.ONNX_LIFTS` keys.
  graphopt has `test_onnx_codec_drift.py` for the Python side; the
  Rust port can silently miss a future liftmap addition. Recommend a
  generated key-list parity test (the liftmap is data — cheap to
  diff).

### mlopt `7cc431d`/`ec6b5e7`/`022e936` — CLEAN.

- `7cc431d`/`ec6b5e7` docstring fix is *more* honest: `textenc`
  correctly reclassified as lift-declining (`pack_padded_sequence`
  not `torch.export`-able), removed from the recipe's `--ref` list.
- `022e936` (delivery prior v2, `WIN_MARGIN = 1.02`) is a
  conservative-direction change: reward is now `committed_speedup`
  (eager/best only when the best *verified* config clears the
  margin, else 1.0), `committed()` ships eager unless the margin is
  cleared, `oracle_pick` applies the same gate, playbook's
  early-stop reads `committed_speedup`. Sub-margin "wins" — bench
  noise — can no longer score or ship. Verify gate untouched:
  `deliver_game.py:507-551` still mandates `lower(verify=…)` per
  config, failed-verify → decline, `unverified` flagged under
  `--no-verify`, and `committed()` notes "shipped WITHOUT the
  equivalence gate" honestly. `DELIVERY_PLAYER_WEIGHTS` remains `{}`
  (cold prior — no stale v1 weights shipped under v2 semantics).
- mlopt `pytest -k "deliver or play"`: 65 passed. Full suite did not
  finish inside 300 s (torch/heavy adapter tests — pre-existing
  suite weight, not a regression signal; subset covering the changed
  code is green).

## Pass-2 findings re-verified (spot check)

- **F7** `reshape_flatten_demo` quarantine — holds (above).
- **F2/F3/F9** verify-gate honesty in `deliver_game.py`/`deliver.py`/
  `stage.py` — holds; the v2 margin change only *tightens* what ships.
- **F5** drift guards — `test_onnx_codec_drift.py`,
  `test_surface_table_drift.py` still present in graphopt.
- **F10** — `eqsat.py` keeps the uncovered-laws reporting and
  named-refusal stats; `improved`-null guard survived the merge.

## New findings

- **P3-N1 (LOW)** — source-onnx: no drift guard ties the 107-entry
  Rust lift table to `mlopt_onnx.liftmap.ONNX_LIFTS`; the completeness
  claim is doc-only. Add a key-parity test (the Python table is
  importable data).
- **P3-N2 (LOW, docs)** — kit-eqsat `lib.rs` `corpus_json()` docstring
  claims its output is "the exact bytes `eqsat_laws.json` should
  contain"; post-`631b889` the emitter renders
  `json.dumps(indent=1, sort_keys=True)` — `corpus_json()` is compact
  `serde_json::to_string`, so the bytes differ (records identical).
  Same staleness in kit-eqsat `AGENTS.md`/"Consumption story"
  ("hand-concatenated snapshot … becomes generated" — it *is*
  generated now). Docs pass needed, same class as pass-2 N1.
- **P3-N3 (INFO, coordination)** — the uncommitted engine-eqsat WIP
  (~78 laws) currently makes `kit-eqsat`'s `corpus_drift` test FAIL
  and will require regenerating `kit-eqsat/data/laws/*` +
  `eqsat_laws.json` before those commits land, or CI goes red. The
  guard is doing its job; flagging so the wave's landing order is
  deliberate: engine-eqsat emit → kit-eqsat copy → eqsat emit.
- **P3-N4 (INFO)** — `corpus_drift.rs` doesn't check for *extra*
  packs in `engine-eqsat/laws/` (iterates `LAW_PACKS` only) and its
  "missing" panic message also covers corrupt-JSON. Both fail loudly
  today; cosmetic.
- **P3-N5 (INFO)** — `emit_eqsat_laws.py --check`/drift test skip
  cleanly when kit-eqsat is absent (documented convention); a stale
  `~/GitHub/handl-lang/kit-eqsat` fallback clone is used silently if
  the sibling is missing — consistent between emitter and test, so
  no self-drift, but the "canonical" could be an old checkout.

## Updated scorecard

| criterion | pass1 | pass2 | pass3 |
|---|---|---|---|
| prod ready | PARTIAL | PASS | PASS |
| documented | FAIL→PARTIAL | PASS− (N1) | PASS− (N1 residual + P3-N2) |
| working | PASS | PASS | PASS (all touched suites green) |
| verified | BIG IMPROVEMENT | PASS | PASS — and *stronger*: the law corpus is now generated + byte-drift-guarded end-to-end |
| tested | PASS | PASS | PASS (new drift tests verified firing, not just present) |
| useful | unchanged | PASS | PASS |
| elegant | IMPROVING | PASS− | PASS− (one codec preserved; kit data owns corpus; P3-N1 coverage nit) |

**Exit assessment**: no invariant violations in the new wave. The
verification story *improved* — the law corpus went from
hand-concatenated snapshot to generated artifact with a working
drift chain in both directions (kit↔engine-eqsat, kit→eqsat), and
the bridge-leaf fixes closed a real phantom-shape soundness hole
before it could ship a non-materialisable extraction. Remaining
items are one LOW coverage nit (P3-N1), one docs pass (P3-N2/N1
residual), and a landing-order note for the in-flight WIP (P3-N3).
