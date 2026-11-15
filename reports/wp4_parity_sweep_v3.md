---
title: WP4 parity sweep v3 — `engine="eqsat"` vs default Python `EGraph` (post-F2)
type: report
date: 2026-11-15
---

# WP4 parity sweep v3 — `engine="eqsat"` vs default Python `EGraph` (post-F2)

**Date:** 2026-11-15 (re-run) · **Gate for:** ADR-0178 WP4 (default engine flip)
**Driver:** `graphopt:bench/drivers/audit/eqsat_parity_sweep.py` (unchanged)
**Prior:** `wp4_parity_sweep_v2.md` — **NO-GO** (3/9 cost parity, 272 uncovered,
`softmax_fold` silenced by the `__tuple__` attr-envelope bug F2)
**Raw JSON:** `wp4_parity_sweep_v3.json` (this directory) + `/tmp/wp4_parity_sweep_v3.json`

## Verdict: **NO-GO** — F2 fixed and verified, parity up to 4/9, two blockers remain

The F2 fix (`engine-eqsat 736f810`, `__tuple__`/`__list__` attr envelopes now
decode engine-side) is **confirmed end-to-end**: `softmax_fold` fires and
delivers **exact cost parity** — eqsat 327 → 65, verified
`max_abs=5.55e-17`, identical to the Python arm.  Scoreboard improved on every
axis except the two known structural gaps:

- cost parity **3/9 → 4/9** (`softmax_fold` joins `silu_fold`/`glu_fold`/`weight_merge`)
- eqsat verified **9/9**, hard failures **0** (unchanged)
- coverage **341 requested / 69 applied / 272 uncovered** (unchanged — see corpus note)
- `select_mul` **still regresses** 15 → 38 under `{"kind":"size"}` extraction (F3 open)

## Environment (what was actually measured)

| component | version |
|---|---|
| Python packages | eqsat main `631b889` overlaid on PYTHONPATH — the **111-record** `eqsat_laws.json` snapshot.  kit-eqsat's corpus has since grown to **243 records / 8 packs**, but the eqsat repin (v0.1.4 cascade) has **not landed**: `eqsat_laws.json` still contains 111 records and eqsat HEAD is still `631b889` (working tree has only an `AGENTS.md` edit).  The sweep therefore measures the 111-record wire corpus — the honest current shippable state. |
| `eqsat-run` (gate arm) | engine-eqsat **clean HEAD `736f810`** (`git status` clean; `cargo build --bin eqsat-run` reported fresh — binary fingerprint matches the F2 commit's tree).  Built in-tree, reusing existing `target/`. |

Reproduce:

```sh
EQSAT=/home/luis/GitHub/handl-lang/eqsat
PYTHONPATH=$EQSAT/packages/mlopt-orchestrator/src:\
$EQSAT/packages/mlopt-core/src:\
$EQSAT/packages/mlopt-dtypes/src \
MLOPT_EQSAT_RUN=$HOME/GitHub/handl-lang/engine-eqsat/target/debug/eqsat-run \
.venv/bin/python bench/drivers/audit/eqsat_parity_sweep.py \
    --out /tmp/wp4_parity_sweep_v3.json
```

## Results

| case | python cost | eqsat v2 | eqsat v3 | eqsat verified | fired eqsat laws | Δ vs v2 |
|---|---|---|---|---|---|---|
| conv_bn_fold | 29186 → 27649 | 29186 → 29186 | 29186 → 29186 | ✅ 0.0 | — | same (law not portable) |
| silu_fold | 194 → 193 | 194 → 193 | 194 → **193** | ✅ 4.4e-16 | `silu_fold` | parity (unchanged) |
| softmax_fold | 327 → 65 | 327 → 327 | 327 → **65** | ✅ 5.6e-17 | `softmax_fold` | **NEW PARITY — F2 fixed** |
| glu_fold | 98 → 33 | 98 → 33 | 98 → **33** | ✅ 0.0 | `glu_fold` | parity (unchanged) |
| swiglu_mlp | 4356 → 4355 | 4356 → 4356 | 4356 → 4356 | ✅ 0.0 | `silu_expand` | same (off-by-1 vs python) |
| select_mul | 15 → 15 | 15 → **38** | 15 → **38** | ✅ 0.0 | `select_mul` | **regression persists (F3)** |
| rms_norm_fold | 282 → 129 | 282 → 218 | 282 → 218 | ✅ 0.0 | `pow_to_square`, `square_expand` | same partial |
| mul_unsq_nat | 75 → 50 | 75 → 75 | 75 → 75 | ✅ 0.0 | — | same (law not portable) |
| weight_merge | 4163 → 2049 | 4163 → 2049 | 4163 → **2049** | ✅ 7.8e-16 | `weight_factor_linear` | parity (unchanged) |

**Scoreboard:** cost parity **4/9** (was 3/9) · eqsat verified **9/9** ·
eqsat hard failures **0** · laws requested 341 / applied 69 /
**uncovered 272** (unchanged — measured against the 111-record wire corpus;
the 243-record kit-eqsat corpus is not yet pinned into eqsat).

## F2 spot-check — `softmax_fold` (the v2 victim)

```
python  cost 327.0 -> 65.0   verified=True  max_abs=5.551115123125783e-17
eqsat   cost 327.0 -> 65.0   verified=True  max_abs=5.551115123125783e-17
         fired=['softmax_fold']  rule_fires={'softmax_fold': 1}
         term_ops=['softmax']  stop='fixed_point'  iterations=2
```

The guard that silently declined in v2 (`attr-is RK true` ∧ tuple-`dim`
check) now fires on the first saturation pass.  The delivered term is a
bare `softmax` — bit-near-exact parity with the Python arm, not just
cost parity.

## Coverage accounting (identical every case)

`rules="default"` resolves to **341** rule names; the portable
`LAW_FORMAT = 2` corpus shipped in `eqsat_laws.json` is **111 records**
(69 applied ∩ requested, 272 requested uncovered — `conv_bn_fold`,
`rms_norm_fold`, `mul_unsq_nat`, the minted `prim:synop_*` folds, and the
procedural-hook/conv tail).  The kit-eqsat corpus grew 111 → **243
records / 8 packs**, but until the eqsat repin lands the sweep can only
measure the 111-record wire set.  When it does land, `laws_applied` should
grow — `conv_bn_fold`/`rms_norm_fold`/`mul_unsq_nat` portability is the
open question that decides whether the coverage blocker narrows.

## Remaining blockers (ranked)

1. **Coverage gap: 272/341 requested laws uncovered (80%)** — unchanged.
   The paying composite laws (`conv_bn_fold`, `rms_norm_fold`,
   `mul_unsq_nat`) still have no portable record *in the shipped 111-record
   snapshot*.  Pending the kit-eqsat 243-record repin this may partially
   close; re-run after it lands before re-judging.
2. **F3 — `select_mul` worse-than-input regression (15 → 38).**  The law
   fires and verifies, but extraction prices `{"kind":"size"}` while the
   case's axis is launch cost — a verified delivery *worse than doing
   nothing*.  Needs a `CostSpec`→pipeline-cost mapping or a
   never-worse-than-input extraction guard.
3. **Partial-parity cases:** `rms_norm_fold` (218 vs 129 — fires helper
   laws but not the fold), `swiglu_mlp` (4356 vs 4355 — off-by-one),
   `conv_bn_fold`/`mul_unsq_nat` (no fire — coverage, not bugs).

Resolved since v2: **F1** (mul_unsq_nat crash — still resolved), **F2**
(`__tuple__`/`__list__` attr envelopes — fixed at `736f810`, confirmed by
this sweep).

## Recommendation

**NO-GO** for flipping `engine=None → "eqsat"` as the default — but the
gap is now two items, not three:

1. Land the kit-eqsat 243-record repin into eqsat and re-run this sweep —
   if `conv_bn_fold`/`rms_norm_fold`/`mul_unsq_nat` become portable, the
   coverage blocker may collapse to a residual tail.
2. Fix the extraction cost-spec asymmetry (F3) — `select_mul` is a
   measured worse-than-input delivery.

Soundness is clean (9/9 verified, 0 failures), parity is 4/9 and rising,
and the F2 root cause is confirmed dead.  Non-local passes remain
`skipped (engine is not an EGraph)` on every case — structural, unchanged.
