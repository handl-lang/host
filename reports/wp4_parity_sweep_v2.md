---
title: WP4 parity sweep v2 — `engine="eqsat"` vs default Python `EGraph`
type: report
date: 2026-11-15
---

# WP4 parity sweep v2 — `engine="eqsat"` vs default Python `EGraph`

**Date:** 2026-11-15 (re-run) · **Gate for:** ADR-0178 WP4 (default engine flip)
**Driver:** `graphopt:bench/drivers/audit/eqsat_parity_sweep.py` (unchanged)
**Prior:** `wp4_parity_sweep.md` — **NO-GO** (2/9 cost parity, 272 uncovered,
1 hard failure)
**Raw JSON:** `wp4_parity_sweep_v2.json` (clean-HEAD binary) and
`wp4_parity_sweep_v2_wipbinary.json` (WIP-inclusive binary — identical output)

## Verdict: **NO-GO** — improved, still not a drop-in

The soundness gate now passes cleanly: **9/9 eqsat deliveries verified,
zero hard failures** (the `mul_unsq_nat` extraction crash is fixed by the
`__mlopt_leaf__` shape-decode commits).  Cost parity improved to **3/9**
(`glu_fold` and `weight_merge` newly match), but three blockers remain:

1. **Coverage gap unchanged: 272/341 requested laws uncovered (80%).**
   `conv_bn_fold`, `rms_norm_fold`, `mul_unsq_nat` — the paying composite
   laws — still have no portable record.
2. **New root-caused bridge bug:** mlopt's `__tuple__`/`__list__` attr
   envelopes are opaque to engine-eqsat guards — `attr-type`/`attr-len`/
   `attr0`/`attr-eq`-to-literal and `dspec` exprs over them all decline.
   `softmax_fold` is portable, applied, pattern-matches, and **can never
   fire** on real lifts (`dim=(-1,)` arrives as `{"__tuple__":[-1]}` →
   `V::Obj`, not `V::Seq`).  Proven by guard-ablation below.
3. **Extraction cost-spec asymmetry now produces a real regression:**
   `select_mul` delivered 15 → **38** (worse than its own input) because
   extraction prices `{"kind":"size"}`, not the case's launch cost.

## Environment (what was actually measured)

| component | version |
|---|---|
| Python packages | eqsat **main `631b889`** overlaid on PYTHONPATH (orchestrator/core/dtypes) — the 111-record `eqsat_laws.json` snapshot. **Note:** graphopt's `v0.1.3` pin resolves to `84f059c`, which *predates* the refresh (tag carries the old 55-record file); the sweep measured the committed HEAD overlay, same convention as v1. |
| `eqsat-run` (gate arm) | **clean HEAD `308443b`** built in a detached worktree `engine-eqsat-clean/` (`git worktree add --detach … 308443b`, `cargo build --bin eqsat-run -j 2`) — the dirty-tree WIP (`+3282` lines: batch_norm/conv_folds/prim_folds packs, new guard preds) is *excluded*. |
| `eqsat-run` (WIP arm) | `engine-eqsat/target/debug/eqsat-run` @ dirty tree — ran the identical sweep; **every row identical** (the WIP's new predicates/shape-exprs are unused by the 111-record corpus; its new packs are builtin-side and never sent over the wire). |

Reproduce:

```sh
EQSAT=/home/luis/GitHub/handl-lang/eqsat
PYTHONPATH=$EQSAT/packages/mlopt-orchestrator/src:\
$EQSAT/packages/mlopt-core/src:\
$EQSAT/packages/mlopt-dtypes/src \
MLOPT_EQSAT_RUN=$HOME/GitHub/handl-lang/engine-eqsat-clean/target/debug/eqsat-run \
.venv/bin/python bench/drivers/audit/eqsat_parity_sweep.py \
    --out docs/reports/wp4_parity_sweep_v2.json
```

## Results (clean-HEAD binary; WIP binary identical)

| case | python cost | eqsat cost | eqsat verified | fired eqsat laws | Δ vs v1 |
|---|---|---|---|---|---|
| conv_bn_fold | 29186 → 27649 | 29186 → 29186 | ✅ 0.0 | — | same (law not portable) |
| silu_fold | 194 → 193 | 194 → **193** | ✅ 4.4e-16 | `silu_fold` | parity (was parity) |
| softmax_fold | 327 → 65 | 327 → 327 | ✅ 0.0 | — | same cost, **new diagnosis** (see F2) |
| glu_fold | 98 → 33 | 98 → **33** | ✅ 0.0 | `glu_fold` | **NEW PARITY** — law now fires (guard fix) |
| swiglu_mlp | 4356 → 4355 | 4356 → 4356 | ✅ 0.0 | `silu_expand` | same |
| select_mul | 15 → 15 | 15 → **38** | ✅ 0.0 | `select_mul` | **NEW REGRESSION** — worse than input |
| rms_norm_fold | 282 → 129 | 282 → 218 | ✅ 0.0 | `pow_to_square`, `square_expand` | same partial |
| mul_unsq_nat | 75 → 50 | 75 → 75 | ✅ 0.0 | — | **crash fixed**; pad laws now correctly decline; `mul_unsq_nat` still not portable |
| weight_merge | 4163 → 2049 | 4163 → **2049** | ✅ 7.8e-16 | `weight_factor_linear` | **NEW PARITY** — different law path, same delivered cost |

**Scoreboard:** cost parity **3/9** (was 2/9) · eqsat verified **9/9**
(was 8/9 + 1 hard fail) · eqsat hard failures **0** (was 1) ·
laws requested 341 / applied 69 / **uncovered 272** (unchanged).

## Coverage accounting (identical every case)

`rules="default"` resolves to **341** rule names (core `DEFAULT` 134 +
carrier preset ~207 — `default_rules()` only reaches 341 once
`mlopt_carriers` is imported).  The portable `LAW_FORMAT = 2` corpus is
now **111 records** (generated from the canonical `kit-eqsat` corpus by
`eqsat/scripts/emit_eqsat_laws.py`, drift-guarded):

- **69 records applied** — names ∩ requested (includes the newly
  portable `softmax_fold`, `glu_fold`, `select_mul`, `swiglu_fuse`,
  `mul_unsq_pad_*`, `sdpa_fold_*`, `weight_*` families).
- **42 corpus records not requested** under default rules —
  `comm_add`/`assoc_*`/`sdpa_expand_*`/`sigmoid_fold`/`tanh_fold`/
  `hardsiglu_fold`/`pow_*`/`linear_*_scale`/`qkv_fuse`/`swiglu_fuse`/
  `parallel_mul_fuse`/`sub_const_*` etc. (they'd only ride under other
  presets such as `default_attention`).
- **272 requested names uncovered** — `conv_bn_fold`, `rms_norm_fold`,
  `mul_unsq_nat`, the minted `prim:synop_*` folds, and the procedural-
  hook/conv tail.  Same honest-gap figure as v1: the corpus grew
  55 → 111 records but the *requested-side* intersection stayed 69.

## New finding — F2: `__tuple__`/`__list__` attr envelopes are opaque to guards

`softmax_fold` is in the portable set, is applied, and its LHS
`div(exp(u), sum(exp(u), dim=RD, keepdim=RK))` matches the lifted term
exactly — yet `fired=[]`.  Guard ablation on the real lifted root
(`EqsatEngine(laws=[record])`, `engine-eqsat-clean` binary):

| cond variant | fired? |
|---|---|
| original (`attr-is RK true` ∧ (`attr-type RD int` ∨ (`attr-type RD tuple` ∧ `attr-len RD ==1`))) | **no** |
| `["attr-is","RK",true]` only | **yes** |
| `["attr-type","RD","tuple"]` | no |
| `["attr-type","RD","int"]` | no |
| `["attr-len","RD","==",1]` | no |
| `cond: null` | **yes** |

Root cause: `_kterm` encodes attrs as `_wire_value(attr_to_data(v))`, so
a tuple attr crosses as the JSON **object** `{"__tuple__": [-1]}`.
engine-eqsat's `V::from_json` maps objects → `V::Obj`; `AttrKind::Tuple`
requires `V::Seq`, `attr-len`/`attr0` only read `V::Seq`, and `dspec`
exprs (`attr0 RD` → `SD`) decline on `V::Obj`.  Every tuple/list attr —
`dim=(-1,)` on `sum`/`mean`/`softmax`-family ops is pervasive in real
lifts — is invisible to the guard algebra.  This is the same bridge-
encoding bug class as `__mlopt_leaf__` (fixed by `b9fe227`/`308443b`)
but for attrs; it is **conservative** (declines, never unsound fires)
yet silently suppresses laws — `softmax_fold` is the proven instance,
and any applied record whose guard reads a tuple/list attr is suspect.

## Other findings

- **F1 (resolved):** the v1 `mul_unsq_nat` hard failure is gone —
  75 → 75, verified.  The phantom-scalar shape decode was firing
  `mul_unsq_pad_l`/`_r` spuriously; post-fix their guards correctly
  decline (`fired=[]`), and no unloweable term is produced.  The case
  still doesn't optimize because `mul_unsq_nat` itself isn't portable.
- **F3 (new):** `select_mul` regression — the law fires, extraction
  under `CostSpec {"kind":"size"}` picks the select-of-product member,
  which costs **38** on the launch axis vs 15 for the input.  Verified
  but *worse than doing nothing* — the cost-spec mismatch (v1 finding
  #4) now has a measured victim.  A flip needs either a cost-spec
  mapping (`flops`/`op_table`) or a never-worse-than-input extraction
  guard.
- **F4:** `weight_merge` parity is real but indirect — eqsat reached
  2049 via `weight_factor_linear`, not the Python path; delivered
  module verified bit-near-exact (7.8e-16).  Parity of *outcome*, not
  of mechanism.
- **F5:** `softmax_fold`'s non-fire is fully explained by F2 — not a
  coverage gap.
- Non-local passes still `skipped (engine is not an EGraph)` on every
  case — structural, unchanged.

## Recommendation

**NO-GO** for flipping `engine=None → "eqsat"` as the default.  The
trajectory is right — soundness is now clean and parity doubled — but
the gate's own criteria aren't met:

1. **Decode `__tuple__`/`__list__` attr envelopes engine-side** (or
   stop wrapping them on the wire) — until then every tuple-attr guard
   silently declines; `softmax_fold` is the demonstrated loss and the
   blast radius is every `dim=(…)`-guarded law.
2. **Close or accept the 272-law coverage gap** — the paying folds
   (`conv_bn_fold`, `rms_norm_fold`, `mul_unsq_nat`) are still
   Python-only; a default flip silently drops them.
3. **Fix the extraction cost spec** — `size` vs the caller's `cost_fn`
   already produced a worse-than-input delivery (`select_mul` 15 → 38).
   Map `CostSpec` to the pipeline's pricing or gate extraction at
   "never worse than the lifted input".

Re-run command: see *Environment* above.  WIP note: the uncommitted
engine-eqsat WIP does not change this sweep's outcome (identical rows
under both binaries) — its new guard predicates and dspec shape exprs
are outside the 111-record corpus's vocabulary, and its new law packs
are builtin-side, never sent over the eqsat-run wire.
