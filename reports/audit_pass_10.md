---
title: Audit pass 10 — eqsat v0.1.8 propagation + ADR-0178 P3 Steps A–C (stage v3, kit-mlopt shim, emit canonicalization) + law-parity allowlist emptying
type: report
date: 2026-11-16
---

# Audit pass 10

Scope: the eqsat v0.1.8 propagation wave — gamekit v0.1.9,
discoverykit v0.1.12, graphopt v0.1.13, kernelopt + mlopt repins,
mlopt's dropped `[tool.uv] override-dependencies` (mlopt `8c75a06`);
P3 Step A stage records v3 / `SearchResult.eg` nullable (eqsat
`8779a8f`, mlopt `3728651`, kit-prelude `542f17b`); P3 Step B mlopt
Rust-workspace deletion + kit-prelude `hnl-ext-mlopt` shim
(mlopt `928e4cb`, kit-prelude `2d3f265`); P3 Step C `emit._OPS`
canonicalization (mlopt `23fabc8`); law-parity allowlist emptying
(eqsat `0c59def`/`1e531b5`/`a5ca4c2`/`064418c`, engine-eqsat
`3664c9a`, kit-eqsat `d8390f4`/`91f3bda`/`afb9aa9`/`8343887`).
Adjacent same-window commits also reviewed: mlopt `22883df`
(`MLOPT_HNL_RUN_TIMEOUT`/`MLOPT_EQSAT_TIMEOUT`), `a215b78`
(half-precision safetensors upcast).

**WIP present and not disturbed:** eqsat, engine-eqsat and kit-eqsat
carry uncommitted changes (parity-test rework, `builtin.rs`/emit_laws
regen in flight, an untracked `data/laws/scan.json`); host-tensor has a
dirty `Cargo.lock`.  All verdicts are against committed state; tags
were verified against `ls-remote`, locks against committed pyprojects.

## Per-commit verdicts

### eqsat

- **`8779a8f` (SearchResult.eg nullable) — CLEAN.** `eg` widens to
  `Engine | EGraph | None` (`pipeline.py:226`); `alternatives`,
  `frontier` (transitively via `alternatives`) and `certificate` all
  route through `_require_egraph()` (`pipeline.py:237-258`), which
  raises a named `TypeError` for `None` *and* for non-`EGraph`
  engines — strictly better than the previous `AttributeError` on
  eqsat-engine results.  `cache.py:509-515` replay now carries
  `eg=None`/`root_eid=-1` — the honest handle.  Verified `lower()`
  never reads `res.eg`: the only engine touch is
  `_propagate_bounds(stats, res.eg, …)` (`bounds.py:604-605`), which
  isinstance-guards (`rules = _bound_rules(eg) if isinstance(eg,
  EGraph) else {}`, `bounds.py:470`) and fails closed through the
  documented `block_input` fallback — identical behaviour to the old
  fabricated empty `EGraph`.  Targeted pytest
  (`test_ported_optimize_edges`, `test_backend_neutral`,
  `test_ported_compositional_cache`, `test_ported_search_lower_split`)
  green; ruff clean on the touched files.
- **`0c59def`/`a5ca4c2`/`064418c`/`1e531b5` (parity comparator +
  empty allowlist) — CLEAN.** The attrs canonicalisation is
  conservative in the right direction: `{"lit": {record}}`
  whole-attrs envelopes unwrap to per-name literals (the same surface
  Python's flat kwargs produce under exact-keyset matching), while
  `{"bind": …}` whole-payload binds still canonicalise to sentinels
  the Python side never produces — they can only ever land in
  `_ATTR_DIVERGENT`, never silently match.  `derivation` now compares
  as provenance consistency (kit-minus-py must be empty) — under-
  claiming provenance is honest, over-claiming is still flagged.
  `_ATTR_DIVERGENT`/`_FIELD_DIVERGENT` are both empty and the guard
  still runs: `pytest tests/test_law_bank_corpus_parity.py` — **5
  passed** against the real kit-eqsat corpus (not skipped).

### engine-eqsat / kit-eqsat

- **`3664c9a` (qkv_fuse attrs_fields) — CLEAN.** `reshape`/`sdpa`
  attr binds move from whole-payload `attrs_bind` to per-key
  `attrs_fields({shape|scale: bind})` — *stricter* matching (exact
  keyset), closing the last Python-bank ↔ corpus divergence.  The
  `scale` metavar `SC` is still shared across the linear and sdpa
  nodes, preserving the equal-scale constraint.  Coverage can only
  shrink (extra attrs now refuse the match), never over-match.
- **`d8390f4`** is the generated-corpus sync of the same change —
  byte-consistent with the `builtin.rs` constructors per
  `91f3bda`'s drift guard.
- **`91f3bda` (corpus_drift repurpose) — CLEAN.** The old guard went
  vacuous when engine-eqsat retired its `laws/` packs; the new one
  pins kit-internal consistency (`LAW_PACKS` ↔ `data/laws/*.json`
  set equality + byte-identity, `LAW_FORMAT = 2` fields, per-pack
  counts) and keeps exactly one cross-repo check — engine-eqsat's
  `laws/` must contain *only* `reshape_flatten_demo.json`.  Cross-repo
  semantic drift stays owned by eqsat's parity suite (verified green
  above).  No honesty surface weakened; the demo pack stays
  engine-only and quarantined.
- **`8343887`/`afb9aa9`/`d0439d3`** — corpus sync + docs; nothing
  executable changed beyond the record payload.

### graphopt / gamekit / discoverykit / kernelopt

- Repin commits only (`graphopt d7ad241`, `kernelopt 4b402c8`,
  `gamekit 694bbee`, `discoverykit 4f62b3e`).  Verified every
  `[tool.uv.sources]` pin across all consumer repos resolves to the
  audited commits: remote tags `eqsat v0.1.8 = 8779a8f`,
  `gamekit v0.1.9 = 694bbee`, `discoverykit v0.1.12 = 4f62b3e`,
  `graphopt v0.1.13 = d7ad241` — each tag's commit *is* the audited
  HEAD, and every `uv.lock` resolves to those SHAs.  No stale tag
  pins anywhere in the workspace (swept all `pyproject.toml`s).
  `kernelopt`'s `board.py` `env.eg` is its own locally-owned
  `EGraph` (already `None`-guarded at `:216`,`:242`), not
  `SearchResult.eg` — unaffected.

### mlopt

- **`3728651` (stage v3) — CLEAN mechanics, one gate break (P10-N1).**
  - *Encode:* `_term_encode` → `mlopt_discovery.hnl_codec.
    term_to_json` — produces real `mlopt_core.ir` objects on decode
    (`Op.make`/`Var`/`Param`/`Const`, same classes `lower()`
    consumes), `dag.program` sharing preserved through the
    memoized node table.  `RECORD_VERSION` 2→3; every emitted record
    (including `ok:false` declines and v0 passthroughs) is stamped
    `"v": 3` (`stage.py:926`).
  - *Decode:* `_term_decode` dispatches on top-level payload shape —
    any of `{Op, Arr, Const, Opaque}` routes to `term_from_json`,
    everything else to legacy `term_from_data`.  Verified the
    envelopes are truly disjoint: legacy `term_to_data` top-level keys
    are `op`/`var`/`param`/`const`/`mvar`/`__tag__`/`nodes`/`root`/
    `args`/`attrs` — case-sensitive, zero overlap with the codec set.
    Mixed records (v3 stamp + legacy payloads, e.g. the tamper-test
    fixtures at `test_stage.py:187,259`) decode correctly — pinned by
    tests.  `ir`/`term`/`params` payloads each dispatch
    independently, so a half-migrated record still decodes.
  - *eg=None:* `_rebuild_result` drops the fabricated
    `EGraph(truncation_level=1)` for `eg=None`/`root_eid=-1`
    (`stage.py:554-559`); `lower()` never reads it (verified above),
    and the bound-propagation path isinstance-guards — an
    `error_budget` search replayed through `verify`/`deliver` takes
    the documented `block_input` estimate exactly as the old empty
    `EGraph` did.  Fail-closed preserved.
  - *Tests:* `pytest tests/test_stage.py tests/test_emit_surface.py`
    — **30 passed**; legacy-spelling decode inside v3 records is
    explicitly exercised.
  - **P10-N1 (MEDIUM): the commit leaves `ci.sh py` red.**
    `ruff check src tests` → `E501 Line too long` at
    `tests/test_stage.py:259` (the appended `# legacy spelling, see
    above` comment overflows 100 cols); `ruff format --check src
    tests` → 2 files would be reformatted (`src/mlopt/stage.py:185`,
    `tests/test_stage.py:259`).  Both hunks are from `3728651`.
    `bash ci.sh py` fails at HEAD.
- **`8c75a06` (repin + override drop) — CLEAN.** `[tool.uv]` now
  carries only `[tool.uv.sources]`; the override-dependencies block
  is gone and `uv.lock` resolves the whole graph to the audited tag
  commits (eqsat `8779a8f`, graphopt `d7ad241`, gamekit `694bbee`,
  discoverykit `4f62b3e`).  Upstream pins now agree with mlopt's
  direct pins, so nothing needed overriding.
- **`928e4cb` (Rust workspace deletion) — CLEAN.** `Cargo.toml`,
  `crates/hnl-ext-mlopt/`, and the `rust-check`/`rust-test` ci lanes
  are gone; `ci.sh` is honestly relabelled Python-only; AGENTS.md /
  README point at `kit-prelude` for the ext build.  No dangling
  references to the deleted crate in shipped code.
- **`23fabc8` (emit canonicalization) — CLEAN, semantics-identical by
  construction.** Audited the mechanism end to end:
  - `_MECH_TAGS` is populated by the `_u1`/`_u2` factories at `_OPS`
    build time; verified programmatically that every `_OPS` key
    equals its factory's op argument (no key/op mismatches), so
    `_canonicalize_mechanical`'s `_OPS[name] = _surface_build(...)`
    can never create a spurious entry or miss a rename.
  - Resolution requires `tensor.<name>` present in
    `host_surface.json`, status ∈ {Ported, Handwritten} (`Seam`
    excluded — the honest boundary), and `_tensor_slots(signature)`
    == tagged arity where the signature's LHS must be *all* bare
    `Tensor` slots.  The parser is conservative: `Value...`
    variadics, `Tensor, Tensor` comma spellings, mixed
    `Tensor * Int`, empty LHS, and trailing-`*` forms all decode to
    `None` → candidate keeps its factory.  Verified live: 77 specs
    resolve (53 `Tensor -> Tensor`, 21 `(Tensor * Tensor) ->
    Tensor`, plus `numel`/`item`/`is_nonzero` whose non-Tensor
    *results* are correctly ignored — LHS-only inspection is the
    right call for emission); the only unresolved candidates are
    `zeros_like`/`ones_like`/`copysign` (`Value...` signatures) —
    exactly the pinned kept set.  (The commit message's "14
    candidates" counts the 11 `_bin_scalar_rhs` ops, which were never
    `_u1`/`_u2` candidates — message imprecision only, mechanics are
    consistent.)
  - Generated builders are character-identical in behaviour to the
    factories they replace: same `tensor.<name>(…)` spelling, same
    `_tens` scalar→rank-0 wrapping, surplus operands ignored, attrs
    unread, `IndexError` → named `UnsupportedOpError` at the render
    site (`emit.py:2363-2369`).  Uncovered ops still decline by name —
    the reverse guard (`REGISTRY - _OPS`) is unchanged and green.
  - `_host_surface()` reads the artifact via `find_spec` without
    executing `mlopt_host_tensor` (import-light claim holds);
    `*.json` is declared package-data so the wheel carries it; a
    missing/unreadable artifact yields `{}` → empty `_MECH_SPECS` →
    the drift guard's `assert _MECH_SPECS` fails loudly rather than
    silently degrading.
  - `test_mechanical_specs_resolve_from_the_surface` pins both
    directions (every spec re-resolves; kept-factory set is exactly
    `{zeros_like, ones_like, copysign}`).
  - `pytest tests/test_emit.py tests/test_emit_surface.py` — **76 +
    30 passed** (emit suite incl. surface probes over the generated
    builders).
- **`22883df` (timeout knobs) — CLEAN.** Both env vars are float
  seconds, read at call time, non-positive/unparsable → named
  `ValueError`; `TimeoutExpired` → `KitEvalError` naming
  `MLOPT_HNL_RUN_TIMEOUT`; unset keeps 600 s/300 s bit-identical.
  `partial(EqsatEngine, timeout=…)` is a zero-arg factory consumed by
  `_resolve_engine`'s `engine()` call at search time — binary
  resolution timing unchanged.  Report notes the override.
- **`a215b78` (half-precision upcast) — CLEAN with one disclosure
  nit (P10-N4).** BF16 safetensors decode u16→f32 by bit-widening
  (exact); F16/BF16 param tensors upcast to f32 at the shared
  `_param_tensor` seam — value-lossless (f16/bf16 ⊂ f32) and required
  by torch's dtype check against f32 probes.  Previously BF16
  declined by name and F16 died inside verify — the new path is
  strictly more capable and the module docstring states the
  convention.  See P10-N4 for the notes gap.

### kit-prelude

- **`542f17b` (`v` schema pin) — CLEAN with one asymmetry (P10-N2).**
  `check_record_schema` gates both directions: input records before
  spawn, stage outputs before they flow on.  Missing `v` = v0 legacy
  accepted; `as_i64` rejects bools/floats/strings → named `EvalError`;
  `v > 3` → named `EvalError` telling the user to upgrade.  Negative
  ints pass — same as mlopt's `v > RECORD_VERSION`-only check
  (consistent).  Mirrors `RECORD_SCHEMA_VERSION = 3` =
  `RECORD_VERSION = 3`.  Fixture stages cover accept/future/malformed;
  `cargo test -p handl-kit-mlopt -p hnl-ext-mlopt` — **20 tests, all
  green** (ran locally).
- **`2d3f265` (run_mlopt + hnl-ext-mlopt shim) — CLEAN, fidelity
  verified line-by-line against the deleted crate.** Identical
  semantics: `$MLOPT_PYTHON` → `python3` fallback (the shim *adds* a
  `std::env::var` fallback between the two — a strict superset,
  documented, harmless since `.envs(env)` inherits anyway), argv =
  `-m mlopt` + verbatim args, caller cwd, caller env +
  `PYTHONUNBUFFERED=1`, `status.code().unwrap_or(1)` exit mapping,
  spawn failure → `code: 2` + the same diagnostic.  The manifest is
  unchanged (same `mlopt` command, same help string).  Workspace
  wiring is correct: `members` gains the crate and
  `[patch."https://github.com/handl-lang/kit-prelude"]
  handl-kit-mlopt = { path = "crates/kit-mlopt" }` makes the ext build
  against the *local* kit-mlopt rather than the git dep its manifest
  declares — downstream git consumers still resolve the published
  sibling.  `Cargo.lock` confirms `hnl-ext-mlopt → handl-kit-mlopt`
  resolves workspace-locally.

## Findings

| ID | Severity | Where | What |
|----|----------|-------|------|
| P10-N1 | MEDIUM | `mlopt/tests/test_stage.py:259`, `mlopt/src/mlopt/stage.py:185` | `3728651` leaves the `ci.sh py` gate red: `ruff check` → E501 (trailing comment overflows 100 cols); `ruff format --check` → both files unformatted. Reproduced: `ruff check src tests` = 1 error, `ruff format --check` = 2 files. Trivial mechanical fix (`ruff format` + wrap), but it is a code-format change so left for the parent per audit scope. |
| P10-N2 | LOW | `mlopt/src/mlopt/stage.py:331-333` vs `kit-prelude/crates/kit-mlopt/src/lib.rs:check_record_schema` | `"v": null` asymmetry: mlopt's `rec.get("v") → None → return` treats explicit null as missing v0 and accepts; the kit's `Some(Value::Null).as_i64() → None → Err` refuses it as malformed. A hand-built record passes `mlopt stage` directly but dies in the kit pipeline. Both directions fail loud or accept — no wrong-answer path — but the readers disagree on what "missing" means. Recommend mlopt mirror the kit (`v is None` only when key absent, or reject non-int null). |
| P10-N3 | INFO | `mlopt/src/mlopt/stage.py:394-404` (`_optimizer`) | The `MLOPT_EQSAT_TIMEOUT` knob (22883df) reaches `optimize_model` only; `mlopt stage search` builds `Optimizer` without `engine=`, so stage-driven searches stay at the 300 s default with no env override and no record note. Large-model stage searches can hit the bound silently relative to the CLI path (the eqsat-run timeout error itself is still named). |
| P10-N4 | INFO | `mlopt/src/mlopt/adapters/onnx_rust.py:272-285`, `arch_rust.py:382` | a215b78's f16/bf16→f32 param upcast is value-lossless and verify-convention-correct, but `OptimizeReport.notes` never discloses it — the only record is the original dtype inside `leaf_manifest`/census (`onnx_rust.py:692-700`, `arch_rust.py:541-557`). A delivered `.hnl` artifact of an f16 checkpoint computes in f32; recommend a `report.notes` line when any param dtype was widened, matching the existing leaf/input-provenance notes (`optimize.py:170-190`). |
| P10-N5 | INFO | `graphopt/packages/mlopt-discovery/src/mlopt_discovery/hnl_codec.py:371-387` | `_dec` accepts multi-tag dicts (`{"Op": …, "Arr": …}`) and silently decodes as `Op`, dropping the `Arr`. `term_to_json` never emits this and both engine-eqsat/source-onnx already share the behaviour — pre-existing codec semantics, now on the stage-record path. Only reachable via hand-edited records; the verify gate still arbitrates semantics. Noted for completeness. |
| P10-N6 | INFO | eqsat / engine-eqsat / kit-eqsat worktrees | Uncommitted WIP in all three (parity-test rework, `builtin.rs`/`emit_laws.rs` regen in flight, untracked `kit-eqsat/data/laws/scan.json`); host-tensor `Cargo.lock` dirty. Committed state audited clean; the next pass should re-check these once landed — especially whether `scan.json` changes the parity census. |

No HIGH findings.  No honesty-invariant violations: every new
decline/gate path names its cause, the verify gate is unchanged
(`lower(verify=True)` default intact, `VerifyDeclinedError` still
enforced), the emit canonicalization is semantics-identical to the
factories it replaces, and the kit shim forwards argv/env/exit codes
verbatim.

## Scorecard

| Invariant | Result |
|-----------|--------|
| Verified-or-declined | **Holds.** Stage `ok:false` records still exit 1 with named errors; `v`-schema violations are named `EvalError`/`_StageError` on both sides; emit's uncovered ops still decline by name (reverse guard green); `_require_egraph` declines by name instead of `AttributeError`; kit `run_stage` refuses newer/malformed `v` before spawn *and* on output. |
| No fabricated data | **Holds.** `eg=None`/`root_eid=-1` replaces a fabricated empty `EGraph` — strictly more honest; `cache.py` replay same. BF16/F16→f32 upcast is value-exact and documented (P10-N4 is a disclosure nit, not a correctness one). hnl_codec leaf records carry real shapes/names; `dag.program` preserves sharing exactly. |
| No silent fallback | **Holds.** `_canonicalize_mechanical` can only swap in a builder that emits the identical call — unresolved candidates keep factories, and any surface/signature drift fails the both-directions pin loudly. `_host_surface()` failure → empty specs → guard assertion, never silent degradation. `run_mlopt` spawn failure → `code: 2` + diagnostic, never a fake success. |
| Guards actually guard | **Mostly holds — one red gate.** Targeted suites green: mlopt stage+emit_surface 30, emit 76, optimize_report+cli+contracts 31; eqsat parity 5 + nullable-eg neighbours 44; kit-prelude cargo 20. **But `ci.sh py` is red at mlopt HEAD** (P10-N1: ruff E501 + format). Pin/tag consistency fully verified against remotes. |

**Verdict: PASS with one MEDIUM** — all five scope areas preserve the
honesty invariants; the emit canonicalization is provably
behaviour-preserving and the stage v3 codec dispatch is sound in both
directions.  P10-N1 (red ruff/format gate from `3728651`) should be
fixed before the next release tag; P10-N2/N3/N4 are small honesty/
consistency nits worth tracking.
