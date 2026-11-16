---
title: Audit pass 11 — D-1a kit-onnx split + WP2 law ports (layout 77, prior_art §2 tail 31) + gpt2 key_prefix + T-101 WIP
type: report
date: 2026-11-16
---

# Audit pass 11

Scope: the D-1a dissolution wave — kit-onnx repo (commits `9af4c6d` +
`b81572a`), graphopt `fdf0f76` (split + repin), mlopt `0898cbd`
(docs), source-onnx `b2529be` (oracle refs), eqsat `f4d6754` (guard
names), gamekit `7121851`, kernelopt `81cc951`; the WP2 law-port wave
— engine-eqsat `eb33eb6` (layout pack, 77 laws) + `b745441`
(prior_art 25→56), kit-eqsat `deb40c0`/`f1bc801` (corpus), eqsat
`437872b`/`0e895c9`/`6c6689c` (snapshot 301→409 + port pins);
source-onnx `709a426` (`key_prefix`); mlopt `1fcb1e3` (P10-N1
format fix).  Adjacent WIP audited as-is: source-onnx T-101
(`maximum(scaled,1)` clamp + `num_buckets >= 4`, uncommitted).

**WIP present and not disturbed:** eqsat carries an uncommitted
`eqsat_laws.json` regen (409→442 records) + `eqsat.py`/test edits;
engine-eqsat `src/builtin.rs` + `tests/contracts.rs` dirty;
kit-eqsat `prior_art.json` 56→89 + `lib.rs`/`corpus_drift.rs`/docs
dirty; source-onnx `src/arch.rs`/`tests/` dirty (the T-101 diff);
host-tensor `Cargo.lock` dirty.  All verdicts are against committed
state unless marked WIP; the WIP sides are mutually consistent
(committed kit-eqsat corpus totals exactly 409 = committed eqsat
snapshot; WIP sides both move to 442/89).

## Per-commit verdicts

### kit-onnx / graphopt split (D-1a)

- **kit-onnx `9af4c6d`/`b81572a` — CLEAN mechanics, two gate gaps
  (P11-N1/N2).** Subtree split preserves history; dist + import
  renamed `mlopt-onnx`/`mlopt_onnx` → `kit-onnx`/`kit_onnx`
  throughout `src/`.  `b81572a` scopes the ruff gates to `src/`
  (tests carry pre-existing import-order drift — same convention as
  graphopt).  `pytest tests/` — **41 passed**; `ruff check src` +
  `ruff format --check src` clean.  Tag `v0.1.0` exists on the
  remote at `b81572a` = HEAD (`git ls-remote` verified).
- **graphopt `fdf0f76` — CLEAN rename, one red gate (P11-N1).**
  `packages/mlopt-onnx` deleted; root dep `mlopt-onnx` → `kit-onnx`
  pinned `{ git, tag = "v0.1.0" }`; `uv.lock` resolves to `b81572a`.
  Import-linter `mlopt_onnx` layer removed from `layers` and
  `root_packages`; the "domain packages never reach the game layer"
  forbidden contract is unchanged (kit_onnx was never a source
  module there).  Coverage `[tool.coverage.run] source` correctly
  drops `mlopt_onnx` (now 4 packages).  `mutmut.sh`,
  `complexity_baseline.json`, `manifest/kit.toml`, ci.yml comments
  all consistent.  The five cross-backend test files
  (`test_onnx_{sink,source,liftmap,codec_drift}` +
  `test_dtype_registry`) import `kit_onnx` and pass — **379 passed**
  in 10.9 s.
- **Stale-reference sweep — CLEAN.** Org-wide grep for
  `mlopt_onnx`/`mlopt-onnx` across `*.toml/*.py/*.rs/*.cfg/*.yaml`
  (excluding `.venv`, `docs/vendored`, `reports/`): zero live-code
  hits.  Remaining mentions are historical docs/reports describing
  the split, plus one cosmetic tempfile prefix
  `mlopt-onnx-` at `mlopt/src/mlopt/adapters/onnx_rust.py:578`
  (P11-N7).  eqsat `f4d6754` renames the import-linter forbidden
  modules (`kit_onnx`) in `eqsat/pyproject.toml:100`; gamekit
  `7121851` same at `gamekit/pyproject.toml:79`.
- **mlopt `0898cbd`, source-onnx `b2529be`, kernelopt `81cc951` —
  CLEAN.** Doc/comment/oracle-path renames only; no dep changes
  (none of these repos ever pinned `mlopt-onnx`).  source-onnx's
  `liftmap_drift.rs` oracle path now resolves
  `../kit-onnx/src/kit_onnx/liftmap.py` — sibling exists.
- **kernelopt repin status — CORRECT AS-IS.** kernelopt pins
  graphopt `v0.1.13` subdirs (`mlopt-torch`/`-carriers`/`-discovery`)
  which are pre-split and self-consistent; its `uv.lock` contains no
  `mlopt-onnx`/`kit-onnx` (nothing in its dep tree needs the ONNX
  backend).  Same for mlopt.  graphopt `fdf0f76` is **untagged** — no
  `v0.1.14` exists; harmless while consumers pin subdirs, but the
  next graphopt release should tag the split (P11-N6).

### WP2 law ports (layout 77 + prior_art §2 tail)

- **engine-eqsat `eb33eb6` (layout, 77 laws) — CLEAN, cond
  translations faithful.** Spot-checked six translations against
  `eqsat/packages/mlopt-core/src/mlopt_core/laws/layout.py`'s
  procedural hooks and `typing.py`'s `_axis_pair`/`_broadcast`/
  `_matmul_shape`:
  - `cond_commute_unary` → `["axes-distinct","x","D0","D1"]`
    (`builtin.rs:~6213`) ≡ `_check_commute_unary`: tuple shape +
    rank ≥ 2 (inside `axes_pair`, `shape.rs:2917`) + distinct
    normalised pair; dimmed attrs range-checked `[-rank,rank)` and
    `rem_euclid`-normalised — identical to `_axis_pair`
    (`typing.py:1438`).  Bare spellings take the last-two default on
    both sides; bound-`null` attrs decline on both (the Rust
    `missing()` check distinguishes unbound from bound-null exactly
    like Python's `in bound` + `_axis_pair` non-int refusal).
  - `cond_commute_binary` adds `["rank-eq","a","b"]` ≡ the
    `len(sa)==len(sb)` equal-rank guard; both shapes must be tuples
    (`rank_eq` fails closed on `Unknown`, `shape.rs:2740`).
  - `cond_involution` → `["axes-eq","x","I0","I1","O0","O1"]` ≡
    `_check_involution` — both pairs distinct AND equal as sets
    (`shape.rs:2956` implements exactly that).
  - `cond_transpose_matmul` → `["rank",…,">=",2]` ×2 +
    `["axes-last2",["mm-out","A","B"],"D0","D1"]` ≡
    `_check_transpose_matmul`: `mm-out`→`matmul_shape`
    (`shape.rs:513`) reproduces `_matmul_shape`'s batch broadcast +
    contraction check with `None`-dim wildcards (`dim_eq`,
    `shape.rs:475`); `Invalid`/unresolvable → `Unknown` → decline,
    matching `_INVALID`/non-tuple refusals.  Output rank feeds
    `axes-last2` exactly like Python's `len(batch)+2`.
  - `cond_mm_transposes`/`cond_linear_shapes`/`cond_mm_t_is_linear`
    — `axes-last2` per operand, `rank`/`dim-compat` for the linear
    contract (`x[-1]` vs `W[1]`, `None`-wildcard both sides).  The
    hardcoded rank-2 in `_check_mm_t_is_linear` is matched by
    `rank W == 2` + `axes-last2` on W.
  - Pack census verified: 64 commute (48 unary `LAYOUT_UNARY_OPS` +
    16 binary `LAYOUT_BINARY_OPS` — the sorted projections of
    `POINTWISE_UNARY_OPS`/`POINTWISE_BINARY_OPS`), 4 involution, 6
    matmul, 3 NT-bridge = 77; opt-in `layout()` pack, never in
    `all()` — matches `ALL_RULES_WITH_LAYOUT` semantics.  Shared
    `D0`/`D1` attr metavars on binary operand transposes preserved
    (identical-swap constraint).  `mul2_to_add`-style `derivation`
    provenance carried (`with_derivation`).
- **engine-eqsat `b745441` (prior_art 25→56) — CLEAN.** The §2 tail
  is mostly *already-declarative* in Python (`cond=` tuples, no
  `check=` hooks — the eqsat commit correctly notes "no check→cond
  translations in this slice").  Spot-checked `abs_idem`,
  `sqrt_square`, `maximum_idem`, `tanh_neg`/`cos_neg`/`exp_neg_recip`/
  `log_exp`/`log_recip`, the scalar folds (`recip_recip`,
  `add_neg_to_sub`, `sub_neg_to_add`, `neg_sub`, `sub_zero_to_neg`,
  `id_sub`, `id_div`, `add_self`, `mul2_to_add`, `mul_neg1`) and the
  `_zeros_like` family: patterns, Const leaves (`Pat::constant`),
  tags (`prior_art`+`simplification`), `with_derivation("add_self")`
  all verbatim.  `unit_like_rule`'s cond is a *literal port* of
  Python's own `cond=("and",("shaped",("bcast","u","v")),
  ("shape-eq","u",("bcast","u","v")))` — identical guard, not a
  re-derivation.
- **kit-eqsat `deb40c0`/`f1bc801` + eqsat `437872b`/`0e895c9`/
  `6c6689c` — CLEAN.** Committed corpus totals verified: 10 packs,
  **409 records** (layout.json=77, prior_art.json=56, prim_folds=60
  included); eqsat's committed `eqsat_laws.json` = 409 — byte-consistent
  generation path (`emit_eqsat_laws.py` concatenates `LAW_PACKS`
  order).  `_CHECK_TRANSLATED` grew 36→113 names (36 attention + 77
  layout); `_ATTR_DIVERGENT`/`_FIELD_DIVERGENT` still empty;
  `test_check_translated_records_match_the_translation_shape` pins
  the exemption shape both directions.  Port-backlog docstring
  corrected to 195 Python-only rules.
- **Parity — GREEN.** `pytest tests/test_law_bank_corpus_parity.py
  tests/test_eqsat_laws_corpus.py` — **8 passed** (run against the
  WIP working tree where both sides sit at 442/89 — consistent;
  committed 409/409 was the state at commit time).  kit-eqsat
  `cargo test --workspace` — all green incl. `corpus_drift` guards.

### source-onnx `709a426` (gpt2 `key_prefix`)

- **CLEAN with one footgun noted (P11-N3).** `KeyPrefixSpec`
  `{strip, add}` with `deny_unknown_fields`; spec validation refuses
  an empty block and empty-string strip/add; `checkpoint_key()`
  (`arch.rs:1597`) composes strip-then-add, strip is prefix-only.
  All three store-lookup sites route through it:
  `verify_weights` leaf loop (`:1652`) + alias loop (`:1675`),
  `bind_weights` leaf loop (`:1715`) + alias loop (`:1755`) — no
  bypass path (grep-verified: the only other `.source` uses are
  manifest emission, which correctly stays logical).  Bound payload
  keys are leaf *names* (unique by `leaf()`'s duplicate refusal), so
  the remap can't collide payload keys; manifests/`leaves_json`
  report logical sources — honest.  `gpt2_key_prefix_remaps_-
  checkpoint_keys` + `key_prefix_validation_refusals` tests pass;
  `gpt2_small_binds_real_safetensors` (env-gated, real checkpoint)
  exercised in the t5 run below.  **Gap:** no injectivity check —
  two logical sources differing only by the strip prefix (e.g.
  `transformer.foo` and `foo`) would silently bind the same tensor;
  `verify_weights`' dtype/`source_shape` check catches it loudly
  unless both leaves declare identical dtype+shape.  Spec-authoring
  hazard only; the committed gpt2 spec has no such pair (P11-N3).

### T-101 WIP (source-onnx, uncommitted)

- **`maximum(scaled,1)` clamp — proof verified airtight.** The
  dead-element argument: `scaled < 1.0` (computed) ⟹ `rel_abs <
  max_exact` (int64), because `rel_f`/`max_exact_f` are exactly
  representable and IEEE round-to-nearest is monotone with `1.0`
  representable — a true quotient ≥ 1 can never round below 1.0, so
  every clamped element satisfies `is_small = rel_abs < max_exact`
  (`arch.rs:4095`) and is discarded by `where`.  Live elements
  (`rel_abs ≥ max_exact` ⟹ `scaled ≥ 1.0`) pass through
  bit-identically — `maximum` is the identity there.  Boundary
  `rel_abs == max_exact` → `scaled == 1.0` exactly → clamp no-op,
  `log(1)=0` → `rel_if_large = max_exact`, matching HF.  The
  `rel_abs == 0` diagonal (the actual `log(0) = -inf` trigger) is
  dead via `0 < max_exact ≥ 1`.  `num_buckets >= 4` validation
  (`arch.rs:1386`) keeps `max_exact ≥ 1` for the bidirectional case
  (`eff = n/2`, `max_exact = eff/2`); for the decoder `eff = n` so
  `n ∈ {2,3}` would also have been safe but is refused — over-strict
  by ≤3 buckets, honest named refusal (P11-N4).  `tensor.maximum`
  exists (`host-tensor/src/ops/mod.rs:164,415`; mlopt `emit.py:1661`
  maps it).  Residual: `max_distance == max_exact` → `denom = 0` →
  `log/0` → NaN/inf → loud `tensor.to` cast failure — unvalidated
  but fail-loud, pre-existing (P11-N4).  `cargo test --test arch t5`
  — **5 passed** incl. real-safetensors bind; op-census test updated
  to expect the two new `maximum` ops.

### mlopt `1fcb1e3` (P10-N1 fix) — CLEAN.

`ruff check src tests` → all checks passed; `ruff format --check` →
38 files already formatted.  The pass-10 MEDIUM is resolved.

## Findings

| ID | Severity | Where | What |
|----|----------|-------|------|
| P11-N1 | MEDIUM | `graphopt/.github/workflows/ci.yml:105,111-114` vs `graphopt/pyproject.toml:284` | **Coverage gate divergence → CI red.** `fdf0f76` adds `kit_onnx` to the ci.yml `coverage run --source` list, so `coverage report --omit="*/mlopt_discovery/*"` measures it under `fail_under=100` — but the two coverage-driver test files (`test_onnx_opmap.py`, `test_onnx_backend_registry.py`) moved to kit-onnx. Measured: all five remaining `kit_onnx`-touching test files (379 tests, the complete set — grep-verified) leave `kit_onnx` at **96%** (`opmap.py` 92%, `lower.py` 96%, `arrays.py`/`report.py`/`__init__.py` <100). `coverage report` exits nonzero → CI red at HEAD. Meanwhile `[tool.coverage.run] source` correctly *excludes* `kit_onnx`, so `scripts/gates.sh`'s full lane never measures it — local gate green, CI red. Compounding: the pyproject comment claims kit-onnx's "coverage floor lives there now", but kit-onnx has **no** `[tool.coverage.report] fail_under` and its `ci.sh` runs no coverage at all — the floor exists nowhere. Fix: either omit `*/kit_onnx/*` from the ci.yml report (matching the pyproject source list), or give kit-onnx a real floor and accept the graphopt wall excludes it. |
| P11-N2 | LOW | `kit-onnx/pyproject.toml`, `kit-onnx/ci.sh` | The "leaf by contract" claim (`AGENTS.md`, `__init__.py:25`) is unenforced: kit-onnx has no `[tool.importlinter]` contract and no coverage floor, and graphopt removed `mlopt_onnx` from its layers contract — nothing machine-checks that `kit_onnx` stays torch/`mlopt_torch`-free. A stray backend import would pass every kit-onnx gate. Cheap fix: a two-line forbidden contract (`source_modules=["kit_onnx"]`, `forbidden_modules=["torch","mlopt_torch","mlopt_carriers","mlopt_orchestrator"]`) + `lint-imports` in ci.sh. |
| P11-N3 | LOW | `source-onnx/src/arch.rs:1597` (`checkpoint_key`) | No injectivity check on the remap: logical sources `transformer.foo` and `foo` both resolve to `foo` under `strip="transformer."` and would silently share one checkpoint tensor when dtype+`source_shape` coincide (`verify_weights` catches mismatched declarations loudly). Current specs unaffected; consider an assemble-time duplicate-`checkpoint_key` refusal. |
| P11-N4 | INFO | `source-onnx/src/arch.rs:1386,4135` (WIP) | (a) `num_buckets < 4` refusal is over-strict for decoder-only specs (`eff = num_buckets`, `num_buckets ∈ {2,3}` → `max_exact = 1`, valid) — honest named refusal, noted. (b) `max_distance == max_exact` → `denom = ln(1) = 0` → `log/0` NaN/inf → loud `tensor.to` failure; unvalidated but fail-loud and pre-existing — a `max_distance > max_exact` spec check would name it earlier. |
| P11-N5 | INFO | eqsat / engine-eqsat / kit-eqsat / source-onnx worktrees | Uncommitted WIP in all four (corpus regen 409→442 with prior_art 56→89, `builtin.rs`/`contracts.rs` edits, T-101 clamp). Committed state verified consistent (409=409, parity green on the WIP tree); next pass should re-audit once landed — especially whether the +33 prior_art records carry new cond translations. |
| P11-N6 | INFO | `graphopt` | Split commit `fdf0f76` is untagged — remote tags stop at `v0.1.13` (= `d7ad241`, pre-split). All consumers pin `v0.1.13` *subdirectory* packages which remain self-consistent (no consumer needs `kit-onnx` transitively — verified in mlopt/kernelopt `uv.lock`s). Tag `v0.1.14` at the next release so the split state is pinnable. |
| P11-N7 | INFO | `mlopt/src/mlopt/adapters/onnx_rust.py:578` | Tempfile prefix still `mlopt-onnx-` — cosmetic stale name only. |

No HIGH findings.  No honesty-invariant violations: every new
decline path names its cause (`key_prefix` validation, `num_buckets`
refusal, guard declines fail closed through `Unknown`/`Invalid`),
the verify gate is untouched, the law-port cond translations are
equal-or-stricter versus the Python hooks (the only asymmetry found
is `axes_pair` requiring rank ≥ 2 where `_axis_pair` allows rank 1 —
moot, since `_bound_axes` guards `rank < 2` first), and the T-101
clamp's dead-element proof holds under IEEE monotone rounding.

**Verdict: PASS with one MEDIUM** — the D-1a split is mechanically
clean and fully pinned (`kit-onnx v0.1.0` = `b81572a` remote-verified,
zero stale refs, guards renamed in eqsat/gamekit), the WP2 ports are
faithful, and `key_prefix` can't mis-bind on any committed spec.
P11-N1 should be fixed before the next graphopt CI run — the
coverage wall currently measures `kit_onnx` at ~96% under
`fail_under=100` while the config's own `source` list excludes it;
P11-N2/N3 are small gate/spec-hardening nits worth tracking.
