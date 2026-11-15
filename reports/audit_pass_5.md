---
title: ADR-0174/0176/0178 audit — pass 5 (eqsat default-engine flip + mlopt emit/arch wave + source-onnx arch assembler)
type: report
date: 2026-11-15
---

# Audit pass 5

Scope: the commits landed since pass 4 — eqsat `039c9c5` + `dd2cac1`
(**the default engine flip to `"eqsat"`** — the biggest change in the
wave) and `6574c86` (eqsat_laws.json regen to 243, closing P4-N1);
mlopt `6e7dfac`/`f700b64`/`c18c9e4`/`56f0e9c`/`4710f41`/`3e5f579`/
`4169ddf`/`3cc9e29` (arch + onnx-rust adapters, verify-run pre-lifted
references, conv1d/permute/carrier-expand/hash-cons/leaf-dedup emit
work); source-onnx `cb06dfe`/`4d5f18a`/`d061817` (arch assembler +
decoder + `weights.rs`).  kit-prelude `f4f9b56` was already in the
pass-4 list — skipped per instructions.

**Uncommitted WIP present and not disturbed:** source-onnx has
`M src/arch.rs`, `M tests/arch.rs`, `?? data/architectures/
bert-minilm-l6.json` (a BERT-family assembler in progress — the
worktree `arch.rs` is ~130 lines ahead of `d061817` with a `bert`
family arm).  The audit below is against the *committed* state; the
`cargo test` run necessarily exercised WIP+commits together (all
green — see below).  eqsat/mlopt worktrees were clean.

## Per-commit verdicts

### eqsat

- **`6574c86` (eqsat_laws.json → 243 records) — CLEAN, closes P4-N1.**
  `tests/test_eqsat_laws_corpus.py` + `tests/test_eqsat_engine.py`:
  **23 passed** — the drift guard that was red at pass-4 HEAD is green.
  Docstrings updated to the 243-record / 8-pack corpus (the
  simplification/categorical/prior_art/attention/pointwise_factor/
  batch_norm/conv_folds/prim_folds list matches kit-eqsat's
  `LAW_PACKS`).  Commit message's parity claims (rms_norm_fold exact
  parity; swiglu_mlp residual = the skipped nonlocal pass) are
  consistent with the engine semantics verified below.
- **`039c9c5` (default engine flip) — CLEAN.** Verified in the diff
  *and* live:
  - `_DEFAULT_ENGINE_NAME = "eqsat"` (`optimize.py:366`);
    `_resolve_engine` maps `None`→`"eqsat"`, `"python-egraph"`/
    `"python"`/`"egraph"`→`EGraph`, unknown strings→named `ValueError`
    (`optimize.py:367-405`).  Lazy import preserved — constructing
    `EqsatEngine` is where the binary resolves.
  - **Missing `eqsat-run` → named `EqsatUnavailableError`, never a
    fallback** — confirmed live with `MLOPT_EQSAT_RUN=/nonexistent`:
    `search(...)` raised `EqsatUnavailableError: MLOPT_EQSAT_RUN=…
    is not an executable file`.  The message names the
    `engine="python-egraph"` opt-out (`eqsat.py:197-205`).
  - **`engine="python-egraph"` opt-in works** — live smoke:
    `search(m, …, engine="python-egraph")` → `stats["engine"]="python"`,
    term extracted.
  - **Default path runs eqsat for real** — live smoke with the
    debug `eqsat-run` binary present: `search(m, …)` (no engine) →
    `stats["engine"]="eqsat"`, `stats["nonlocal_passes"]="skipped
    (engine is not an EGraph)"`, term extracted.  `stats["eqsat"]`
    is populated inside `EqsatEngine.run` (`eqsat.py` ~line 640:
    `laws.requested/applied/uncovered`, `cost`, `cost_fn`
    portability note, `fired`) — honest, requested-vs-applied-vs-
    uncovered never conflated.
  - **`detect_*` guards actually guard** — `_check_detect_engine`
    (`optimize.py:526-557`) raises `TypeError` naming the flags and
    the `engine='python-egraph'` remedy for *any* non-`EGraph` engine
    when `detect_factors`/`detect_specials`/`detect_headshare` is on;
    it sits on the single `search()` path `Optimizer.search`
    delegates to (`optimize.py:894`), so there is no bypass.
    `error_budget`'s pre-existing `_check_budget_engine` is
    untouched.
  - **`discover_*` pins the reference engine** —
    `discover_alternatives` gains `engine: Any = "python-egraph"`
    (`optimize.py:1693`) and `Optimizer.discover` setdefaults
    `engine="python-egraph"` only when `self.engine is None`
    (`optimize.py:1662-1667`) — an explicit `Optimizer(engine=…)`
    still wins, and frontier methods (`alternatives()`/
    `certificate()`/`diverse_classes()`) decline on non-EGraph
    engines by their own contract.  Documented in the docstring.
  - Nonlocal passes remain EGraph-gated (`isinstance(eg, EGraph)` at
    `optimize.py:939`) with the skip recorded in stats — no silent
    drop, no pretend-run.
  - Tests: `test_native_engine.py` gained the default-resolution,
    opt-out, eqsat-default, and detect-decline pins; pipeline suites
    pin `engine="python-egraph"` where they exercise Python-engine
    machinery.  `pytest tests/test_native_engine.py -m "not slow"`:
    **7 passed, 14 skipped** (all skips = `mlopt_native` wheel
    absent — expected); the `requires_eqsat` tests ran against the
    real binary.
- **`dd2cac1` (README note) — CLEAN**, docs-only.

### mlopt

- **`6e7dfac` (arch adapter) — CLEAN.** `adapters/arch_rust.py` is a
  subprocess bridge to source-onnx's `arch` CLI: spec resolution
  declines by name when nothing resolves (`_resolve_spec_path`),
  missing `arch` binary is a named `ValueError` listing remedies,
  `weights=` opt validated before spawn, and a spec with neither
  `weights=` nor a `checkpoint` block declines listing the remedies —
  **weights are never synthesized** (`_resolve_weights`,
  `arch_rust.py:221-246`).  Inputs are seeded synthetic probes with
  `input_source ∈ {synthetic, mixed, feed, none}` recorded in spec
  meta — honest provenance, same contract as onnx-rust.
- **`4169ddf` (Rust-side checkpoint resolution + bound payload) —
  CLEAN.** Python no longer locates/transforms checkpoint tensors:
  `--weights`/`--checkpoint` + `--st-out` emit a bound safetensors
  keyed by leaf source with the manifest rewritten to `identity`;
  `_bind_params` re-checks every leaf's post-transform shape against
  the manifest and declines by name on any miss (`arch_rust.py:330-390`).
  `_apply_transform` keeps `transpose_last_two` = `swapaxes(-1,-2)`
  ([out,in]→[in,out] — matches the Rust `apply_transform`, verified
  below) and **raises on any unknown transform name** — no guessed
  semantics.  Conflicting dual-claims on one source tensor refuse
  assembler-side (`arch.rs bind_weights`).
- **`f700b64` (input-probe provenance in report + lift record) —
  CLEAN.** `optimize.py` appends an `input values: synthetic|mixed`
  note naming the `inputs=`/`feed=` remedy; `stage.py` lift record
  carries `input_source` + the same note.  `leaf_source` stays
  accurate (weights ARE real) — the new channel doesn't conflate the
  two.
- **`c18c9e4` (verify-run pre-lifted references) — CLEAN.** For
  data-carrier specs (`OnnxLift`/`HnlLift`/`ArchLift` —
  `spec.module` is a term payload, not callable), `_prelifted_reference`
  rebuilds the torch reference by lowering `SearchResult.ir`
  (the *source* IR — verified: `SearchResult.ir=ir` is the
  pre-search IR at `optimize.py:1004`) through
  `ir_to_torch_module` with `param_values`, falling back to
  `spec.source.to_ir`.  This is exactly the reference
  `lower(verify=True)` compared against — same gate, honest.
  Non-callable + no source IR → printed decline, exit 1;
  non-empty `spec.kwargs` → named decline (IRModule takes positional
  inputs only).  Callable-module specs unchanged.
- **`56f0e9c` (conv1d + permute) — CLEAN with one latent LOW
  (P5-N2).**
  - `conv1d`→degenerate NCHW `conv2d`: unsqueeze H=1 on NCL input and
    (out,in,k) weight, `stride=[1,s]`, `padding=[0,p]`, squeeze back —
    arity/order matches the torch binding
    (`torch_bindings.py:1800`: `conv1d(x, w, bias?, stride=, padding=,
    dilation=, groups=)`); `dilation != 1` declines by name (host
    `conv2d` has no dilation slot).  Correct.
  - `permute`→`swapaxes` chain: literal-permutation check
    (`sorted(d % n) == range(n)`), computed dims decline, missing
    dims → reverse-all-axes via `shape_of` (unknown rank declines).
    Transposition decomposition is correct.
  - **P5-N2 (LOW):** `_permute` reads dims only from attrs or
    `args[1]` — a `permute(x, 0, 2, 1)` multi-positional-arg term
    (which the torch binding `t.permute(*a)` *does* accept) would
    read `dims=0` → emit an identity/wrong permutation **silently**.
    All current lifters mint `dim=` attr or a single list arg
    (verified: `source-onnx/src/lifter.rs` Transpose/NonZero/Gemm
    sites, `mlopt-onnx/liftmap.py`), so this is latent, not live —
    but `emit()` is a public door on arbitrary terms.  Same class:
    `_conv1d_dim`/`_permute` accept floats and truncate via `int()`
    (`stride=1.5` → `1`) where the rest of the surface is strict.
    Recommend: consume `args[1:]` as dims when no attr, and reject
    non-integral floats.
- **`4710f41` (carrier_expand) — CLEAN with one latent LOW (P5-N1).**
  Verified the unfolds are *verbatim* the shipped definitional laws:
  `OM_UNLIFT` (`om_apply(om_elem(s,v)) → matmul(softmax(s,dim=-1),v)`,
  `graphopt:mlopt_carriers/om.py:195-200`) and `OM_DECAYED_UNLIFT`
  (`om_decayed(s,d,v) → matmul(mul(s,d),v)`, `om.py:1487-1493`;
  `_om_decayed_torch` lowers `(s*d)@v` identically, `om.py:1053-1057`).
  Bottom-up structural rewrite, memoized; every other carrier shape
  (`om_compose`, `omd_*`, `aff*`/`apply*`, `trace`/`scan`, decode ops,
  bare `om_elem`) passes through and still declines by name in
  `_OPS` — **no guessed carrier semantics**.  This cannot produce
  wrong code *for canonical terms*: the rewrites are e-class equalities.
  - **P5-N1 (LOW):** the matcher doesn't check `om_apply`'s own
    arity/attrs — `om_apply(x, extra_arg)` or an attr-carrying
    `om_apply`/`om_decayed` would have args/attrs silently dropped by
    the rewrite.  Canonical carrier ops are arity-fixed (shape rules
    in `eqsat:mlopt_core/typing.py:1795-1809`), so unreachable from
    shipped laws — but `emit()` is public.  Recommend an
    `len(args)==1`/`attrs`-empty guard on the `om_apply` arm (and
    attrs check on `om_decayed`).
- **`3e5f579` (hash-consed `__sN ::` bindings) — CLEAN.** Walked the
  algorithm: `_count_refs` counts *parent references* over the DAG
  (each unique node once, iterative — `add(t,t)` counts 2, both use
  sites render the name ✓); `_bind_order` is post-order DFS so every
  bound node's bound descendants land before it (children pushed
  after the expand marker, LIFO → descendants complete first) ✓;
  binding RHSs render through `render` (root in full) while use sites
  go through `go` (shared-name shortcut) — and `_OPS` builders receive
  `go` as their child renderer (`emit.py:1888`), so names propagate
  inside RHSs ✓.  `__sN` names avoid leaf-bound names + `tensor` via
  the shared `taken` set; no hnl keyword collision plausible.  The
  root is never bound; `emit()` stays the unshared door.  Sharing is
  semantics-preserving by construction (same subterm → same name).
  Dead-binding worst case (a counted ref inside an attr payload that
  renders inline) is possible but harmless — an unused `__sN ::` is
  dead code, not wrong code.
- **`3cc9e29` (DAG-aware `leaf_kinds`) — CLEAN.** Iterative single-
  visit walk preserving first-use leaf order; `_prelude` takes the
  precomputed kinds so `emit_function` walks once.  2^22-occurrence
  stress 3.67 s → 50 µs claimed; the randomized-DAG order-equivalence
  is pinned in `tests/test_emit.py`.
- **Tests:** `pytest tests/test_emit.py tests/test_emit_surface.py
  tests/test_arch_rust_source.py tests/test_onnx_rust_source.py` —
  **74 passed** (56 s).  `test_emit_surface` drift-guard still green
  at 201 ops.

### source-onnx

- **`cb06dfe` (whisper-encoder assembler + spec) — CLEAN.** Shapes are
  *computed*, not fabricated: `positions` derives from
  `conv_out_len` (standard `(L+2p−k)/s+1`, dilation fixed 1,
  negative-numerator refusal) applied twice for the two conv
  front-end layers, and **refuses** when positions exceed
  `max_source_positions` rather than clipping (`arch.rs:517-525`).
  `head_dim = d_model / attention_heads`, `attention_scale` defaults
  to `head_dim^-0.5` (HF convention) with spec override.  The
  `mul_scalar(scores, 1.0)` after QK^T is a semantic no-op kept for
  structural parity with the HF spelling (q was already scaled) —
  harmless, noted for completeness.  Every needed op is already in
  the lifter vocabulary; weight leaves carry declared
  `(source, source_shape, transform)` triples verified against the
  checkpoint before binding.
- **`4d5f18a` (decoder + turbo encoder + weights.rs) — CLEAN.**
  - **`weights.rs`**: HF-cache-only resolution (`HF_HUB_CACHE` →
    `HF_HOME/hub` → `~/.cache/huggingface/hub`), shard-index
    (`*.index.json` weight_map) following, "newest snapshot by
    (mtime, name)" matching the Python adapter; **cache miss =
    named `WeightError::NotFound`, never a download**.  safetensors
    dtype coverage is explicit — `F8_E5M2`/`F8_E4M3` and unknown
    dtypes return `None`→named refusal, no approximation.
  - **`apply_transform` (`transpose_last_two`) — verified correct.**
    Element-wise last-two-axes permutation: for `[out,in]` HF Linear
    weights it produces `[in,out]` with the index math
    `src=(o·rows+r)·cols+c → dst=(o·cols+c)·rows+r` — a true
    transpose, byte-width-checked (`bytes.len() == numel·elem`),
    rank≥2 enforced, unknown transforms refuse by name.  Matches the
    Python `_apply_transform` (`swapaxes(-1,-2)`) used on pre-payload
    docs.
  - **`bind_weights`**: every leaf re-checked (dtype name,
    pre-transform shape) — binding implies verification; conflicting
    claims on one source tensor **refuse by name** (one payload key
    can't carry two transforms); alias sources included raw so
    presence checks see the checkpoint's answer.  The decoder's
    tied `proj_out`/`embed_tokens` dual-transform claim is exactly
    the case the commit message says refuses at payload emission —
    pinned in `tests/arch.rs` + spec description.
  - **Refuse-by-name coverage** is broad: unsupported family,
    malformed spec fields, unsupported safetensors dtypes, shape
    mismatches, conflicting binds — all `ArchError::Unsupported`/
    `Bind`/`Spec` variants, no silent fallback anywhere in the diff.
- **`d061817` (docs) — CLEAN**, AGENTS/README only.
- **`cargo test` on the worktree (WIP-inclusive): all green** —
  lib 1, arch 14, weights 12(?), plus doc/other suites; 0 failures.
  (Counts as listed in the run: 1+14+2+1+12+7 across binaries.)

## Findings

| ID | Severity | Where | What |
|----|----------|-------|------|
| P5-N1 | LOW | `mlopt/src/mlopt/carrier_expand.py:60-70` | `om_apply` unfold doesn't check the `om_apply` node's own arity/attrs — a non-canonical `om_apply(x, extra)` or attr-carrying carrier would be silently rewritten, dropping the extra. Canonical carrier ops are arity-fixed so unreachable from shipped laws; latent for hand-built terms fed to `emit()`. Same for `om_decayed` attrs (arity is checked). |
| P5-N2 | LOW | `mlopt/src/mlopt/emit.py:596-628` (`_permute`), `:519-545` (`_conv1d_dim`) | `_permute` reads dims only from attrs/`args[1]` — the torch binding accepts `permute(x, 0, 2, 1)` positional dims, which would emit a wrong (identity/partial) permutation silently. All current lifters mint `dim=`/list args — latent. Also both helpers `int()`-truncate non-integral floats (`stride=1.5`→`1`) where the surface is otherwise strict. |
| P5-N3 | INFO | source-onnx worktree | Uncommitted BERT-family WIP (`src/arch.rs`, `tests/arch.rs`, `bert-minilm-l6.json`) present during audit — not disturbed; `cargo test` ran against WIP+commits (green). Re-audit when it lands. |

No MEDIUM+ findings.  The two LOWs are the same shape as prior-pass
latent-port findings: correct on every term the shipped machinery
produces, silently wrong only on hand-minted terms outside the
canonical arity/attr contract.

## Scorecard

| Invariant | Result |
|-----------|--------|
| Verified-or-declined | **Holds.** verify-run rebuilds the *same* reference `lower(verify=True)` used; arch/onnx-rust declines name every miss (binary, weights, checkpoint, spec, transform, dtype, shape); carrier_expand only applies shipped definitional equalities; unknown `engine=` strings raise `ValueError`. |
| No fabricated data | **Holds.** arch positions/head_dim/scale are computed from spec+checkpoint with refusals; weights never synthesized; `input_source` honestly reports synthetic probes; conv_out_len refuses impossible convs. |
| No silent fallback | **Holds — and strengthened.** The engine flip keeps `EqsatUnavailableError` named (verified live), adds `TypeError` on `detect_*` opt-ins that can't run, `ValueError` on unknown engine names, and stats record `nonlocal_passes="skipped…"` + `stats["eqsat"]["laws"]["uncovered"]` verbatim. |
| Guards actually guard | **Holds.** `_check_detect_engine` sits on the single `search()` path; drift guard green at 243; emit surface drift-guard green at 201; source-onnx safetensors dtype/shape/transform checks all refuse by name. |

**Verdict: PASS** — all commits preserve the honesty invariants; two
LOW latent emit-surface findings (P5-N1, P5-N2) worth a follow-up
hardening pass but not blocking.
