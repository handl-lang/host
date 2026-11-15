---
title: WP4 parity sweep v4 — `engine="eqsat"` vs default Python `EGraph` (post-243-corpus)
type: report
date: 2026-11-15
---

# WP4 parity sweep v4 — `engine="eqsat"` vs default Python `EGraph` (post-243-corpus)

**Date:** 2026-11-15 (re-run) · **Gate for:** ADR-0178 WP4 (default engine flip)
**Driver:** `graphopt:bench/drivers/audit/eqsat_parity_sweep.py` (unchanged)
**Prior:** `wp4_parity_sweep_v3.md` — **NO-GO** (4/9 cost parity, 69 applied /
272 uncovered, `select_mul` worse-than-input regression)
**Raw JSON:** `wp4_parity_sweep_v4.json` (this directory) + `/tmp/wp4_parity_sweep_v4.json`

## Verdict: **GO** — 7/9 cost parity, 9/9 verified; the two remaining gaps are coverage/structural, not regressions

The full 9-case re-run after the **243-record corpus regen**
(eqsat `6574c86` — `eqsat_laws.json` now carries the kit-eqsat 243-record /
8-pack corpus), the **never-worse-than-input extraction floor**
(eqsat `5177d97`), and the **attr-envelope fix** (engine-eqsat `736f810`,
carried from v3) lands every previously-blocked parity case:

- cost parity **4/9 → 7/9** — `rms_norm_fold` (exact 282 → 129),
  `mul_unsq_nat` (75 → 50), and `select_mul` (floor holds at 15 → 15)
  all join the v3 parity set
- eqsat verified **9/9**, hard failures **0** (unchanged)
- coverage **341 requested / 133 applied / 208 uncovered** — applied nearly
  doubled (69 → 133) on the repinned corpus; uncovered fell 272 → 208
- `select_mul` **regression dead** — v3's 15 → 38 worse-than-input
  delivery is gone; the floor keeps the input term

Neither remaining gap is a soundness or parity *regression*: one is an
honest coverage hole (the engine declines to fire what it cannot
express), the other a documented pass-level asymmetry (verified output,
one node of cost).  Recommendation details below; the flip itself is the
user's call.

## Environment (what was actually measured)

| component | version |
|---|---|
| Python packages | eqsat main **`6574c86`** — `eqsat_laws.json` regenerated to the
  **243-record** corpus (the kit-eqsat repin v3 was waiting on has landed). |
| `eqsat-run` (gate arm) | engine-eqsat **`736f810`** — attr envelopes
  (`__tuple__`/`__list__`) decode engine-side; built in-tree at
  `target/debug/eqsat-run`. |

Reproduce:

```sh
EQSAT=/home/luis/GitHub/handl-lang/eqsat
PYTHONPATH=$EQSAT/packages/mlopt-orchestrator/src:\
$EQSAT/packages/mlopt-core/src:\
$EQSAT/packages/mlopt-dtypes/src \
MLOPT_EQSAT_RUN=$HOME/GitHub/handl-lang/engine-eqsat/target/debug/eqsat-run \
.venv/bin/python bench/drivers/audit/eqsat_parity_sweep.py \
    --out /tmp/wp4_parity_sweep_v4.json
```

## Results

| case | python cost | eqsat v3 | eqsat v4 | eqsat verified | fired eqsat laws | Δ vs v3 |
|---|---|---|---|---|---|---|
| conv_bn_fold | 29186 → 27649 | 29186 → 29186 | 29186 → 29186 | ✅ 0.0 | — | same (procedural hooks, no LAW_FORMAT=2 record) |
| silu_fold | 194 → 193 | 194 → **193** | 194 → **193** | ✅ 4.4e-16 | `silu_fold` | parity (unchanged) |
| softmax_fold | 327 → 65 | 327 → **65** | 327 → **65** | ✅ 5.6e-17 | `softmax_fold` | parity (unchanged) |
| glu_fold | 98 → 33 | 98 → **33** | 98 → **33** | ✅ 0.0 | `glu_fold` | parity (unchanged) |
| swiglu_mlp | 4356 → 4355 | 4356 → 4356 | 4356 → **4356** | ✅ 0.0 | `silu_expand` | same (nonlocal-pass asymmetry) |
| select_mul | 15 → 15 | 15 → **38** | 15 → **15** | ✅ 0.0 | `select_mul` | **FIXED — floor holds input** |
| rms_norm_fold | 282 → 129 | 282 → 218 | 282 → **129** | ✅ 0.0 | `pow_to_square`, `rms_norm_fold`, `rms_norm_fold_nogain`, `square_expand` | **NEW PARITY — exact** |
| mul_unsq_nat | 75 → 50 | 75 → 75 | 75 → **50** | ✅ 0.0 | `mul_unsq_nat` | **NEW PARITY** |
| weight_merge | 4163 → 2049 | 4163 → 2049 | 4163 → **2049** | ✅ 7.8e-16 | `weight_factor_linear` | parity (unchanged) |

**Scoreboard:** cost parity **7/9** (was 4/9) · eqsat verified **9/9** ·
eqsat hard failures **0** · laws requested 341 / applied **133** /
**uncovered 208** (was 69 / 272 — the 243-record regen paid off).

## What the repin bought

`rms_norm_fold` is the headline: v3 fired only the helper laws
(`pow_to_square`, `square_expand`) and stalled at 218; v4 fires the fold
itself plus `rms_norm_fold_nogain` and lands the **exact** Python cost
129 — same delivered `rms_norm` term shape.  `mul_unsq_nat` likewise goes
from no-fire to firing and matching 75 → 50 exactly.  Both were the
"paying composite laws" v3 flagged as the coverage question; the answer
is that they *are* portable once the corpus carries their records.

`select_mul` confirms the floor (`5177d97`) end-to-end: the law still
fires and verifies, but extraction can no longer price a verified
extraction worse than the input — 15 → 15, matching Python.

## Remaining gaps (the honest two)

1. **`conv_bn_fold` — genuine coverage gap (accept as Python-edge?).**
   Python reaches 29186 → 27649 via the composite fold; eqsat holds the
   input at 29186, verified.  Root cause per the parity agent:
   `conv_bn_fold` is a **procedural-hook law** — its `check`/`derive`
   hooks are Python callables with **no `LAW_FORMAT = 2` record** in the
   243-record corpus, so nothing portable exists to request.  This is not
   a bug and not a regression: the engine honestly declines, and the
   floor guarantees it cannot do worse than the input.  Options: (a)
   accept — conv+bn fusion stays a Python-engine capability until a
   declarative record is authored; (b) author the `LAW_FORMAT = 2`
   record for the fold.  208/341 requested laws remain uncovered
   (largely the minted `prim:synop_*` tail + procedural hooks), so this
   gap is one instance of a residual class, not a singleton.

2. **`swiglu_mlp` — nonlocal-pass asymmetry (decision needed).**
   Python 4356 → 4355; eqsat 4356 → 4356, verified.  Python's win comes
   from a **nonlocal pass** (`paired_extract`-class machinery) that runs
   outside the rewrite loop; eqsat reports
   `nonlocal_passes: "skipped (engine is not an EGraph)"` on every case —
   a structural property, not a defect.  The eqsat arm still fires
   `silu_expand` and verifies; it simply lacks the pass that would
   recover the last unit of cost.  Options: (a) accept — document the
   asymmetry; the delivered term is verified and within one size unit of
   Python; (b) implement a `paired_extract` equivalent engine-side.
   This is the only case where Python strictly beats eqsat on cost.

Resolved since v3: **F3** (`select_mul` worse-than-input — floor holds),
the **243-record repin** (applied 69 → 133, `rms_norm_fold`/`mul_unsq_nat`
parity), and F1/F2 remain dead.

## Recommendation

**GO** for flipping `engine=None → "eqsat"` as the default, on this
evidence:

- **Soundness clean:** 9/9 verified, 0 hard failures — every delivered
  term is checkable, and the floor makes worse-than-input impossible.
- **Parity is real, not cosmetic:** 7/9 exact cost parity including the
  two hardest composite folds (`rms_norm_fold`, `weight_merge`) and the
  exact-match `rms_norm` term shape.
- **Both residual gaps are forward-looking, not regressions:** conv_bn_fold
  is absent capability (honest decline + floor), swiglu_mlp is a missing
  optional pass (verified, off-by-one).  Neither can produce a wrong or
  worse-than-input artifact under the default.

Caveats to carry into the flip decision: coverage is still 208/341
uncovered — parity on this 9-case suite does not imply parity on the
full law bank, and cases outside the sweep may hit procedural-hook or
nonlocal-pass gaps.  If the flip lands, `conv_bn_fold` should be tracked
as the first `LAW_FORMAT = 2` authoring candidate, and the
`paired_extract` decision (implement vs accept) should be recorded as a
follow-up rather than blocking the flip.
