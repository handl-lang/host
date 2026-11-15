---
title: ADR-0174 exit-criteria audit — pass 4 (kit-eqsat corpus migration wave)
type: report
date: 2026-11-15
---

# ADR-0174 exit-criteria audit — pass 4

Scope: verify the commits landed since pass 3 hold the
honesty/verification invariants — the `__tuple__`/`__list__` attr
envelope decode, the never-worse-than-input extraction floor, the
kit-eqsat corpus migration to 243 records / 8 packs, the
per-artifact wire-prelude cache, the `examples/pipeline` claims, the
source-onnx fixture/lifter fixes + the new Rust↔Python differential
harness, the kokoro `textenc-p` arm, and the `+++` TOML frontmatter
parsing. Uncommitted WIP (eqsat `AGENTS.md`, engine-eqsat `AGENTS.md`,
graphopt lockfiles, host `Cargo.lock`/site drift) was present
throughout and **not disturbed**; no files were modified outside this
report.

## Per-commit verdicts

### engine-eqsat

- **`736f810` (`__tuple__`/`__list__` envelope decode) — CLEAN.**
  `V::from_attr` (`src/guard.rs`) decodes only *single-key* marker
  envelopes with array payloads; malformed payloads and multi-key
  maps stay `V::Obj` and keep declining — verified in the diff
  (`envelope_items` requires `map.len() == 1` and
  `AttrValue::Array`). `V::List` is a distinct variant, so Python's
  `tuple != list` and the `isinstance(v, tuple)` gates
  (`attr-type "tuple"`, `attr-len`, `attr0`, hashability) stay
  exact — `Attr0` indexes `V::Seq` only, `hashable` rejects `List`,
  `v_eq` has no `Seq`/`List` cross-arm. `shape.rs::seq_items` is the
  shared view admitting bare `Array` + both envelopes at every
  attr-read site; `av_truthy` now reads `{"__tuple__":[]}` as falsy,
  matching Python `()`. `to_attr` re-encodes `Seq`/`List` as the wire
  envelopes — the `attr_from_data` inverse, so derived attrs stay
  wire-canonical. Nothing that declined before can fire wrongly: the
  decode only gives previously-invisible sequences their Python
  meaning. `cargo test` on the working tree: all green, including the
  new pins (`kit_softmax_fold_fires_on_wire_tuple_dim`,
  `kit_softmax_fold_declines_on_wire_list_dim`,
  `gqa_absorb_repeat_fires_on_wire_tuple_shapes`).
- **`d2a005a` + `4fecae7` + `4afcf11` (kit-eqsat consumption) —
  CLEAN.** `handl-kit-eqsat` is a **dev-dependency**
  (`Cargo.toml:32`, `[patch]` sibling link at `:55`); the shipped
  crate's corpus remains the `builtin` constructors. `laws/` now
  contains only `reshape_flatten_demo.json` — the UNSOUND demo stays
  engine-local and quarantined exactly as pass 3 verified. Contract
  tests decode packs via `handl_kit_eqsat::law_pack(name).json`
  (`stage4_corpus_files_load_and_match_builtin_packs` passes), so
  kit↔engine drift is impossible by construction — the dep *is* the
  corpus. `emit_laws.rs` remains the regeneration path;
  `prim_folds` has no emit arm by design (minted-record data). The
  two doc commits are docs-only.

### eqsat

- **`5177d97` (never-worse-than-input floor) — CLEAN, and the floor
  is exactly as honest as claimed.** `extract_best`
  (`eqsat.py:705-732`) applies the floor **only** when `cost_fn is
  not None` *and* `eid < len(self._terms)` — `cost_fn=None` returns
  the raw `CostSpec` pick untouched (pinned by the new test). The
  baseline is `self._terms[eid]`, which is correct because `find()`
  is the identity and `run()` records under `root_eid` = the
  `add_term` index (`eqsat.py:576-582,657`) — the recorded input term
  genuinely is the saturated root's member, so pricing both sides
  with one shared `dag_cost` memo restores the Python extractor's
  invariant rather than approximating it. Fallback is recorded on
  the cached result (`fell_back_to_input`/`input_cost`/
  `extracted_cost`), never silent; it can only surrender a better
  `CostSpec` pick, never invent a worse delivery — the docstring
  says so. `pytest tests/test_eqsat_engine.py`: **21 passed**
  (incl. the new floor test covering both directions + the
  `cost_fn=None` skip).
- **Corpus state — FINDING P4-N1 (MEDIUM).** eqsat HEAD (`5177d97`)
  still ships `eqsat_laws.json` at **111 records** while kit-eqsat is
  now **243 records / 8 packs**. `emit_eqsat_laws.py --check` exits 1
  ("drift … rerun scripts/emit_eqsat_laws.py") and
  `tests/test_eqsat_laws_corpus.py::test_shipped_snapshot_matches_kit_corpus`
  **FAILS at HEAD** — verified live. The guard is doing exactly its
  job; this is the landing-order gap P3-N3 predicted: kit-eqsat
  `8343887` landed without the eqsat regen commit. Until
  `emit_eqsat_laws.py` is re-run and committed, eqsat's corpus test
  is red. Not a code defect — a pending artifact regen.

### kit-eqsat

- **`8343887` (243 records, 8 packs) — CLEAN.** Verified by count:
  `simplification` 35, `categorical` 50, `prior_art` 25,
  `attention` 8, `pointwise_factor` 2, `batch_norm` 3,
  `conv_folds` 60, `prim_folds` 60 = **243**; `LAW_PACKS` lists all
  eight in corpus order; `reshape_flatten_demo` asserted absent
  (`law_pack("reshape_flatten_demo").is_none()`,
  `lib.rs:241`). `packs_parse_and_total_243` pins the count.
  `cargo test --workspace`: all green.
- **Drift guard — now VACUOUS (P4-N2, LOW).** `corpus_drift.rs`
  compares kit packs against `engine-eqsat/laws/*.json` "wherever a
  sibling copy still exists" — post-`d2a005a` that dir holds only
  the demo, so the test prints `drift check compared 0/8 packs` and
  **cannot fail on corpus drift anymore**. This is acceptable by
  construction (engine-eqsat consumes the crate as a dep — there is
  no second copy to drift), but the test's remaining value is only
  the demo-exclusion assert; it also still can't see *extra* files
  in `engine-eqsat/laws/` (P3-N4 residual). Recommend either
  documenting it as a transition shim or repurposing it to assert
  the sibling dir contains *only* the demo.
- **`afb9aa9` (P3-N2 docs fix) — CLEAN, closes the finding.**
  `corpus_json()` docstring and AGENTS.md now correctly state the
  contract is record-level equality; byte-exactness lives with
  eqsat's emitter (`json.dumps(indent=1, sort_keys=True)`).
- **`d0439d3` (docs) — CLEAN**, adds the "Adding a pack" procedure.

### kit-prelude

- **`e81f37c` (per-artifact wire-prelude cache) — CLEAN.** The cache
  key is now `artifact_key()` = canonical path + len + mtime-ns of
  the *loaded image containing this code*, resolved via `dladdr` on
  a `static` anchor under `cfg(unix)` (`libc` dep is
  `cfg(unix)`-gated); non-Unix `own_image_path()` returns `None` and
  falls back to `current_exe()` — the pre-fix behavior, honestly
  documented. Old flat `{exe,…}` format fails to deserialize →
  recompute (self-migrating); writes merge per-artifact entries into
  the shared file so one extension's rebuild can't poison another's.
  The new test
  `wire_prelude_cache_is_per_artifact_and_self_migrating` passes
  (verified: planted foreign entry served only to its own key; stale
  flat format ignored). `unsafe` blocks are commented and scoped;
  `dladdr` usage copies `dli_fname` bytes before returning — correct.
- **`f4f9b56` (`examples/pipeline/`) — CLEAN, claims match reality.**
  README preserves the `artifact_executed` caveat verbatim in
  spirit: "smoke-run once through `hnl-run` … NOT a
  numeric-equivalence check", `false` + scoped `notes` when no
  `hnl-run` found. The `>>>` right-associativity caveat and the
  `out`-via-record-spread `deliver` contract are documented and match
  the `.hnl` sources. `delivered.hnl` gitignored.

### source-onnx

- **`d81d52c` (lifter fixes + dev CLI + fixture fix) — CLEAN; the
  fixture fix is semantically right.** `layout.onnx`'s shared `idx2`
  was `[[0,1],[2,0]]`: value `2` is in-bounds for `Gather(g2)` on
  `w[4,3]` axis=0 but **out of bounds** for
  `GatherElements(ge)` on `tiled[2,48]` axis=0 (indices must be `< 2`)
  — invalid ONNX, undefined behavior. `[[0,1],[1,0]]` keeps every
  index `< 2`, valid for *both* consumers. `generate_fixtures.py`
  carries the comment explaining the shared-constant constraint.
  `cargo test layout` → `layout_family_lifts` **passes** on the fixed
  fixture. `const_dtypes.onnx` adds 0-D Constants across
  int/uint/float dtypes; the `const_leaf` fix (unsigned ints now mint
  `Scalar::Int` like Python's `int(arr)` for kinds `"iu"`, `float`
  only for `"f"`) matches the oracle contract in the comment.
  `examples/lift.rs` is a dev-only JSON CLI — refusals serialize as
  `{"ok": false, "error": …}` with exit 1, never a silent partial
  lift. The Gather shape fallback applies ONNX's
  `data[:axis]+idx.shape+data[axis+1:]` rule only when the oracle is
  absent or *fully* unproven — a partially-proven answer is kept,
  never overwritten by a less precise formula.

### graphopt

- **`d5b4ac7` (`onnx_lifter_differential.py`, 1743 lines) — CLEAN;
  verified live, cannot produce false "parity".** Ran it: all 15
  committed fixtures → **9 `parity+eval:equal`, 6 `refuse-parity`,
  0 divergences, exit 0**. The honesty properties hold in code:
  - Term verdict is canonical-DAG isomorphism (root canon-id
    equality over hash-consed `op name + spelling-strict attrs +
    child canon ids`); `_jcanon` tags ints/floats/bools distinctly
    (`0` ≠ `0.0`, `-0.0` ≠ `0.0` via `float.hex()`), `__tuple__`/
    `__list__`/`__nonfinite__` envelopes distinguished.
  - Unevaluated is a first-class verdict: `_Unevaluated` is raised
    for every uncovered op and surfaces as `parity+eval:unevaluated`,
    which is **not** in the pass list (`("parity",
    "parity+eval:equal", "refuse-parity")`) → counted in `bad` →
    exit 1. No path converts "couldn't evaluate" into "equal".
  - Eval uses the *Python* side's input shapes/dtypes for both
    environments, so a Rust-side shape/dtype divergence degrades to
    `eval-error`/`meta-divergence`, never a silent pass; leaf
    payloads are compared by sha256 from each side's own safetensors
    bytes, and the manifest compare covers dtype/shape/nbytes.
  - Refusal parity requires same family AND compatible op; a
    one-sided refusal is `lift-divergence`.
  - Minor (INFO): `_attrs_of` prefers the `attrs` channel over
    `lit.__mlopt_attrs__` if a malformed doc carried both — the codec
    contract is exactly-one, so this is theoretical.
  - Corpus note: the whisper/resnet tiers are env/path-gated and
    skip silently when artifacts are absent — consistent with the
    repo's artifact convention, but "17 models" in the commit message
    includes tiers a fresh clone won't run.

### mlopt

- **`54ebd5a` (kokoro `textenc-p` arm) — CLEAN; port is faithful.**
  Diffed `mlopt/adapters/_kokoro_packed.py` against the proven probe
  `graphopt:bench/drivers/probes/tts_kokoro_packed.py`: the three
  patched forwards, `_register_lstm_op`, and `install` are verbatim
  modulo type annotations, the `_tts_packed_op`→`_mlopt_kokoro_packed_op`
  rename, and `axis=`→`dim=` (documented ty-stub workaround). The two
  semantic notes (`total_length=T` pad tail re-masked downstream;
  per-install op names) are carried verbatim. Arm wiring is honest:
  `packed = arm.endswith("-p")` only reachable via `textenc-p` in
  `_ARMS`; plain `textenc` keeps the `torch.export` decline; `full`
  still walls on `repeat_interleave`; `textenc-p` is correctly
  *not* in `_STOCHASTIC_ARMS`. The "verified, max_abs=0.0 on CUDA"
  claim rests on the bench probe's measured evidence (cited:
  `kokoro_production_bench.md`/`kokoro_e2e_shapes.md`) — not
  re-runnable in this audit (no GPU), but the port fidelity +
  `verify=True` gate downstream make a fabricated claim
  self-defeating. `tests/test_kokoro_adapter.py`: **9 passed**.
  README/AGENTS/scripts doc updates correctly reclassify plain
  `textenc` as lift-declining and scope `textenc-p` to the bench
  measurement.

### host

- **`642a3376` + `96507ba7` (`+++` TOML frontmatter) — CLEAN;
  YAML-wins verified.** All three parsers (`gen-adr-index.py`,
  `check-adrs.py`, `project-frontmatter.py`) use the same shape:
  optional leading `+++` TOML block via `tomllib`, then optional
  `---` YAML block, `data.update(ydata)` last so **YAML wins shared
  keys** — verified live on a synthetic doc (`title`/`status` came
  from YAML, TOML-only `id` preserved, body intact). TOML decode
  failure → empty dict, never a crash; a `+++` block without YAML
  still counts as frontmatter. `check-adrs.py` runs clean of
  frontmatter errors on the migrated corpus (the 2 remaining errors
  are missing `## Decision` sections in ADR-0168/0169 — structural,
  pre-existing, unrelated). `gen-adr-index.py` regenerated
  `INDEX.md` with **zero diff** — the committed index already
  reflects the fixed parsing.

## New findings

- **P4-N1 (MEDIUM)** — eqsat: `eqsat_laws.json` is stale (111 vs the
  canonical 243 records); `test_shipped_snapshot_matches_kit_corpus`
  fails at HEAD and `emit_eqsat_laws.py --check` exits 1. The drift
  guard works — this is the pending regen, not a guard failure.
  Action: run `scripts/emit_eqsat_laws.py` in eqsat and commit.
- **P4-N2 (LOW)** — kit-eqsat `corpus_drift.rs` now compares **0/8**
  packs (verified: "drift check compared 0/8 packs"); post-migration
  it can never catch corpus drift, only the demo-exclusion assert.
  Real protection moved to dep-consumption + eqsat's byte-exact
  guard. Repurpose or document as transition-only; P3-N4's
  extra-files blind spot persists.
- **P4-N3 (INFO)** — graphopt differential harness: `_attrs_of`
  silently prefers `attrs` over `lit.__mlopt_attrs__` if a malformed
  doc carried both; codec contract is exactly-one, so theoretical.
- **P4-N4 (INFO)** — the "17 models / 0 divergences" claim includes
  whisper/resnet tiers gated on local artifacts; a fresh clone runs
  only the 15 fixtures (verified: all pass). Claims are accurate but
  environment-dependent — same convention as other artifact-gated
  tests.
- **P4-N5 (INFO)** — mlopt `textenc-p`'s CUDA `max_abs=0.0` verify
  claim is bench-sourced, not re-verified here; the port is verbatim
  and the downstream `lower(verify=True)` gate is unchanged, so the
  honesty chain holds.

## Pass-3 findings re-verified

- **P3-N2** — closed by kit-eqsat `afb9aa9` (docs now state the
  record-level contract correctly).
- **P3-N1** — closed earlier by source-onnx `6246ddd`
  (`liftmap_drift.rs`); this pass adds the semantic differential
  harness on top.
- **P3-N3** — materialized exactly as predicted: kit-eqsat `8343887`
  landed before the eqsat regen; eqsat is red until the emit lands
  (P4-N1).
- **P3-N4/N5** — residuals noted above (P4-N2 covers N4's extra-files
  blind spot; the stale-fallback-clone convention is unchanged and
  consistent).

## Updated scorecard

| criterion | pass1 | pass2 | pass3 | pass4 |
|---|---|---|---|---|
| prod ready | PARTIAL | PASS | PASS | PASS |
| documented | FAIL→PARTIAL | PASS− | PASS− | PASS (P3-N2 closed; example/pipeline docs honest) |
| working | PASS | PASS | PASS | **PARTIAL→PASS pending regen** — eqsat corpus test red at HEAD until `emit_eqsat_laws.py` lands (P4-N1); every touched suite otherwise green |
| verified | BIG IMPROVEMENT | PASS | PASS | PASS — stronger: Rust↔Python ONNX differential harness verified live (15/15 fixtures honest), floor/decline semantics pinned by tests |
| tested | PASS | PASS | PASS | PASS — new drift/per-artifact/floor tests verified firing, not just present |
| useful | unchanged | PASS | PASS | PASS (textenc-p arm + pipeline example are real capability) |
| elegant | IMPROVING | PASS− | PASS− | PASS− (one-codec discipline kept; vacuous drift test is the only cruft, P4-N2) |

**Exit assessment**: no invariant violations in this wave. The
honesty machinery kept pace with the migration — declines stayed
declines (malformed envelopes, `__list__` dims, plain `textenc`),
the extraction floor is recorded-not-silent, the ONNX differential
harness reports `unevaluated` as failure-not-pass, and the
frontmatter parsers preserve YAML-wins. The single actionable item
is P4-N1: eqsat needs its `eqsat_laws.json` regen committed to turn
the corpus test green again — the guard caught real drift, which is
the system working.
