---
title: WP4 parity sweep — `engine="eqsat"` vs default Python `EGraph`
type: report
date: 2026-11-15
---

# WP4 parity sweep — `engine="eqsat"` vs default Python `EGraph`

**Date:** 2026-11-15 · **Gate for:** ADR-0178 WP4 (default engine flip)
**Driver:** `graphopt:bench/drivers/audit/eqsat_parity_sweep.py`
**Raw JSON:** `/tmp/wp4_parity.json` (reproduce below)

## Verdict: **NO-GO** for the WP4 default flip

The eqsat arm is *sound where it completes* — every extracted term
that lowered verified bit-exact — but it is **not yet a drop-in
replacement**: on this 9-case sweep it matched the Python engine's
delivered cost on only 2 of 9 cases, lost the optimization on 6, and
**failed outright on 1** (`mul_unsq_nat`: the eqsat extraction
lowered to a shape-mismatched module — `RuntimeError: size of tensor
a (4) must match … b (6)` — that the verify gate never got to judge).

## Method

Each case is a `bench.discovery` fixture (spelled "before" module +
fp64 feed, `TorchSource`/`TorchSink`, the case's own `rules`,
`cost` axis, `max_iterations`/`max_enodes`).  Both arms run the
identical call:

```python
search(model, feed, source=TorchSource(), capabilities=TorchSink(),
       cost_fn=COST_FNS[case.cost], rules=case_rules,
       max_iterations=…, max_enodes=…, engine=<None | "eqsat">)
lower(sr, feed, sink=TorchSink(), verify=True)
```

Environment: eqsat **main** (`84f059c`, post-WP3) overlaid on
PYTHONPATH over graphopt's pinned `v0.1.2` orchestrator/core/dtypes;
`eqsat-run` = `engine-eqsat/target/debug/eqsat-run` @ `90a0058`
(`MLOPT_EQSAT_RUN`).  CPU only; sizes 6–16.

## Results

| case | engine | cost before→after (launch axis) | verified | max_abs | search s | notes |
|---|---|---|---|---|---|---|
| conv_bn_fold | python | 29186 → 27649 | ✅ | 8.9e-16 | 1.77 | `conv_bn_fold` law fired (minted composite) |
| conv_bn_fold | eqsat | 29186 → 29186 | ✅ | 0.0 | 0.07 | **no fold** — law uncovered (272/341 requested names have no portable record) |
| silu_fold | python | 194 → 193 | ✅ | 4.4e-16 | 0.03 | `silu_fold` |
| silu_fold | eqsat | 194 → 193 | ✅ | 4.4e-16 | 0.04 | `silu_fold` fired — **parity** |
| softmax_fold | python | 327 → 65 | ✅ | 5.6e-17 | 0.02 | `softmax_fold` |
| softmax_fold | eqsat | 327 → 327 | ✅ | 0.0 | 0.05 | **no fold** — `softmax_fold` uncovered |
| glu_fold | python | 98 → 33 | ✅ | 0.0 | 0.03 | `glu_fold` |
| glu_fold | eqsat | 98 → 98 | ✅ | 0.0 | 0.03 | **no fold** — `glu_fold` uncovered |
| swiglu_mlp | python | 4356 → 4355 | ✅ | 0.0 | 0.03 | pairing/non-local path |
| swiglu_mlp | eqsat | 4356 → 4356 | ✅ | 0.0 | 0.04 | `silu_expand` fired (reverse dir); non-local passes skipped by design |
| select_mul | python | 15 → 15 | ✅ | 0.0 | 0.03 | no-op both arms — parity (trivially) |
| select_mul | eqsat | 15 → 15 | ✅ | 0.0 | 0.04 | |
| rms_norm_fold | python | 282 → 129 | ✅ | 0.0 | 0.04 | full `rms_norm_fold` |
| rms_norm_fold | eqsat | 282 → 218 | ✅ | 0.0 | 0.05 | partial — `pow_to_square`+`square_expand` fired, fold law uncovered; **worse than python, still verified** |
| mul_unsq_nat | python | 75 → 50 | ✅ | 0.0 | 0.04 | `mul_unsq_nat` |
| mul_unsq_nat | eqsat | 75 → ~1e15 | ❌ **lower RuntimeError** | — | 0.05 | `mul_unsq_pad_l`/`_r` fired; extracted term fails to materialise (shape mismatch 4 vs 6). **Hard failure.** |
| weight_merge | python | 4163 → 2049 | ✅ | 7.8e-16 | 0.03 | weight-chain fold |
| weight_merge | eqsat | 4163 → 4163 | ✅ | 0.0 | 0.05 | **no merge** — fold uncovered |

## Coverage accounting (identical every case)

`rules="default"` resolves to **341** rule names; the portable
`LAW_FORMAT = 2` set covers **69** — `laws_uncovered_n = 272`,
matching the census's honest-gap figure.  Fired eqsat laws across
the sweep: `silu_fold`, `silu_expand`, `pow_to_square`,
`square_expand`, `mul_unsq_pad_l`, `mul_unsq_pad_r`.  Every case
recorded `nonlocal_passes = "skipped (engine is not an EGraph)"` —
the pairing/lift asymmetry is structural, not a bug.

## Findings

1. **Soundness where it completes.**  8/9 eqsat deliveries verified
   (`max_abs` 0.0–4.4e-16).  The e-class-equivalence argument holds on
   real lifts.
2. **One hard failure.**  `mul_unsq_nat`: eqsat fired
   `mul_unsq_pad_l`/`_r` (portable records) and extracted a term
   whose lowering crashes on a shape mismatch *before* verify runs.
   Either the pad laws' shape guards under-approximate in
   engine-eqsat, or attrs/shape decode loses information on the wire.
   This is exactly the class of bug the WP4 gate exists to catch —
   and it is in the *portable* set, not the uncovered tail.
3. **Optimization parity is poor.**  EqSat matched python's delivered
   cost on 2/9 (one trivially), lost on 6, crashed on 1.  The misses
   are almost all *uncovered* laws (`conv_bn_fold`, `softmax_fold`,
   `glu_fold`, `weight_merge`, the `rms_norm_fold` composite) plus
   non-local passes — i.e. the 272-law gap is the dominant term, as
   WP3 predicted.
4. **Cost-model asymmetry.**  EqSat extracts under
   `CostSpec {"kind":"size"}`; python extracted under the case's
   `launch_aware_cost`.  `cost_after` above is recomputed on the same
   axis for fairness, but a real flip needs a cost-spec mapping
   (flops/op_table) — `size` is not the pipeline's pricing.
5. **Latency.**  EqSat search was faster on every case (0.03–0.07 s
   vs 0.02–1.77 s) — expected at these sizes; not decision-relevant
   for WP4.

## Recommendation

**NO-GO** for flipping `engine=None → "eqsat"` as the default.
Blocking items, in order:

1. **Investigate the `mul_unsq_nat` extraction failure** — a
   portable-law extraction that produces an unloweable term is a
   correctness bug in the WP2/WP3 bridge or the engine's guard
   port, not a coverage gap.  Reproduce:
   `eqsat_parity_sweep.py --cases mul_unsq_nat`.
2. **Close or explicitly accept the coverage gap.**  272/341
   requested laws uncovered means the default flip silently drops
   most of the bank's paying laws.  Either port the paying subset
   (the folds above are the measurable delta) or keep `engine=None`
   the default and let `engine="eqsat"` stay opt-in until parity is
   demonstrated on the *paying* laws, not just the sound ones.
3. **Decide the extraction cost spec** before any flip — `size` vs
   the caller's `cost_fn` changes what "best" means.

Re-run command (graphopt repo, eqsat-main overlay):

```sh
EQSAT=/home/luis/GitHub/handl-lang/eqsat
PYTHONPATH=$EQSAT/packages/mlopt-orchestrator/src:\
$EQSAT/packages/mlopt-core/src:\
$EQSAT/packages/mlopt-dtypes/src \
MLOPT_EQSAT_RUN=$HOME/GitHub/handl-lang/engine-eqsat/target/debug/eqsat-run \
.venv/bin/python bench/drivers/audit/eqsat_parity_sweep.py \
    --out docs/reports/wp4_parity_sweep.json
```
