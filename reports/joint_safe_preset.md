# Joint-safe law preset — follow-up to I-003 (eqsat joint-pass residual)

Date: 2026-11-16. Follows `eqsat_v0110_perf_check.md` (mitigation #4)
and `layout_law_budgets_fix.md` (budgets landed: 132 s → ~10 s, but the
158-law joint set is still ~2× the v0.1.6 81-law subset's ideal).

Question: does the Compositional `cross_pairs` joint pass need all 219
`_JOINT_DEFAULT` rules, or can a smaller "joint-safe" set preserve
extraction quality at a fraction of the check-time cost?

## Verdict: GO — implemented, with a split the brief didn't expect

The 77 layout laws are **not** one family. Split by name prefix:

- **64 pointwise laws** (`transpose_push_*` / `transpose_pull_*` —
  transpose commuting through elementwise ops): **never changed the
  extracted term on any of 6 captured joint specs.** When they fired
  they only grew the e-graph and the guard-check time — pure amplifier.
- **13 structural laws** (`transpose_transpose_*`,
  `matmul_transpose_rev_*`, `transpose_matmul*`, `linear_*_matmul_t*` —
  involution, product-transpose, `linear` ≡ NT-GEMM): **carry every
  observed layout win.** On transpose-bearing micro-models the
  layout-free set scores 2-3× *worse*; the structural half alone
  reproduces the full set's extraction byte-for-byte.

Dropping **all** layout laws (the report's first suggestion) is
**unsound** — it regresses the transpose cases. Dropping only the
pointwise half keeps 100 % of measured extraction on 6/6 specs.

## Measurements — wire-level A/B on captured joint specs

Specs captured live by spying on `EqsatEngine._invoke` during
`Optimizer(backend=TorchBackend()).optimize(model, x,
strategy=Compositional())` (kit-torch @ `03207247`, eqsat **v0.1.12**,
`MLOPT_EQSAT_RUN=engine-eqsat/target/release/eqsat-run`). Each spec's
`laws` array was filtered to build variants; `options.rule_budgets`
filtered to match (110 budgeted names on the full set). Wall =
`eqsat-run` subprocess, back-to-back runs.

| spec (joint micro-model) | variant | laws (wire) | wall | enodes | cost | term == full |
|---|---|---|---|---|---|---|
| **spec2** DeepParallel(32,48,32)×2 — the report's 16-node capture | full `WITH_LAYOUT−SUBSUMED` | 158 | **9.36 s** | 6 444 | 6.0 | — |
| | − all layout (= v0.1.6 set) | 81 | 0.34 s | 1 374 | 6.0 | ✓ |
| | − layout + 64 pointwise only | 145 | 0.36 s | 1 374 | 6.0 | ✓ |
| | **− SUBSUMED − pointwise (joint-safe)** | **94** | **2.27 s** | 5 420 | 6.0 | ✓ |
| | − layout + 6 fired laws only | 87 | 9.11 s | 6 444 | 6.0 | ✓ |
| **deep64** DeepParallel(64,96,64)×2 | full | 158 | 17.17 s | — | 6.0 | — |
| | v81 | 81 | 0.49 s | 1 374 | 6.0 | ✓ |
| | joint-safe | 94 | 3.32 s | 5 420 | 6.0 | ✓ |
| **tb32** TransformerBlock(32,h4,m2)×2 | full | 158 | 5.17 s | 4 219 | 767.0 | — |
| | v81 | 81 | 0.76 s | 1 084 | 767.0 | ✓ |
| | joint-safe | 94 | 4.61 s | 4 219 | 767.0 | ✓ |
| **pb32** ParallelBlock(32,h4,m2)×2 | full | 158 | 2.67 s | 3 429 | 175.0 | — |
| | v81 | 81 | 0.53 s | 1 040 | 175.0 | ✓ |
| | joint-safe | 94 | 2.75 s | 3 429 | 175.0 | ✓ |
| **tt32** `W(x.t()).t()` Linear×2 — transpose-bearing | full | 158 | 0.044 s | 212 | **2.0** | — |
| | v81 | 81 | 0.009 s | 10 | **6.0** | ✗ |
| | v81 + pointwise only | 145 | 0.013 s | 10 | 6.0 | ✗ |
| | **joint-safe** | **94** | 0.053 s | 212 | **2.0** | ✓ |
| **tsilu** `W(silu(x).t()).t()` Linear×2 | full | 158 | 0.45 s | 2 723 | **4.0** | — |
| | v81 | 81 | 0.011 s | 18 | **8.0** | ✗ |
| | v81 + pointwise only | 145 | 0.017 s | 39 | 8.0 | ✗ |
| | **joint-safe** | **94** | 0.014 s | **62** | **4.0** | ✓ |

Notes on the table:

- **spec2's 158-law set is exactly the v0.1.6 81-law set + the 77
  layout laws** — name-set equality verified programmatically; the
  joint regression is *entirely* the layout pack.
- On spec2/deep64 the 6 layout laws that actually merged
  (`transpose_pull_add`, `transpose_push_add`, `linear_from_matmul_t`,
  `linear_to_matmul_t`, `matmul_transpose_rev_dd`, `transpose_matmul`)
  reproduce the full 6 444-enode closure **without** the other 71 —
  and still cost ~9 s. The e-graph blowup is driven by the
  struct+pointwise *combination*.
- On **tsilu** the pointwise laws fired hard under the full set
  (`transpose_pull/push_{silu,sigmoid,mul}` — enodes 62 → 2 723, a
  43× e-graph) yet the extracted term is identical to joint-safe's.
  This is the amplifier mechanism in its purest form.
- On **tt32/tsilu** the structural laws are *load-bearing*: without
  them nothing fires (enodes 10-39, extraction = input term) and cost
  is 2-3× worse. `v81 + pointwise only` scores identically to `v81` —
  pointwise laws cannot complete a transpose migration without the
  GEMM-side structural laws.
- Contentions caveat: the host was at load-average ~31 during
  re-measurement; absolute walls inflate (a spec2 joint-safe re-run
  measured 5.6-8.3 s vs 2.27 s in the clean batch). All A/B rows above
  are from single back-to-back batches; relative ordering is the
  signal, not the absolute seconds. The e2e recapture below confirms
  the wire set and extraction independently of timing.

## Recommended preset

**`_JOINT_DEFAULT` = `WITH_LAYOUT − SUBSUMED − {transpose_push_*,
transpose_pull_* layout laws}`** = 155 requested rules / **94 applied
on the wire** (81 core + 13 structural layout).

- Preserves extraction on **6/6** captured specs, including both
  transpose-bearing specs where layout-free regresses 2-3×.
- vs the 158-law set: spec2 9.4→2.3 s, deep64 17.2→3.3 s,
  tb32 5.2→4.6 s, pb32 ~par, tsilu 0.45→0.014 s (32×).
- vs the 81-law layout-free set it gives up 2-7× on
  transpose-free specs (the structural laws' residual check cost) —
  the price of keeping the transpose win reachable *by default*.

### Why this narrowing is honest

1. The dropped family is measured dead weight: 0/6 extraction changes,
   and its only observed effect is e-graph/time growth.
2. The kept family is measured load-bearing: 2/6 specs lose 2-3×
   cost without it; the other 4 are extraction-identical with it.
3. The full `WITH_LAYOUT − SUBSUMED` surface remains **opt-in**:
   `_joint_rules` passes any caller-chosen `RuleSet` through as-is, so
   `rules=` callers keep every law.
4. Residual risk, stated plainly: 6 synthetic specs is not a proof.
   A hypothetical IR needing transpose-through-pointwise migration
   that the structural laws cannot complete would lose a rewrite the
   pointwise family could reach — none was found, and the family stays
   reachable via `rules=`.

## Implementation (kit-torch, branch `joint-safe-preset`)

`src/kit_torch/composer.py`:

- `_JOINT_DEFAULT` is now a named `RuleSet("joint_default")` built by
  filtering `WITH_LAYOUT.rules`: drop `SUBSUMED`-tagged and
  `LAYOUT`-tagged members whose name starts `transpose_push_` /
  `transpose_pull_` (`_LAYOUT_POINTWISE_PREFIXES`). Name-prefix is the
  only distinguisher — all 77 layout laws share `LAYOUT`+`EXPANSIVE`
  tags; a corpus sub-tag would be cleaner but needs a kit-eqsat /
  engine-eqsat regen (follow-up, not required for correctness).
- `_joint_rules` docstring updated: the widening is now the
  joint-safe surface; explicit `RuleSet`s (incl. `WITH_LAYOUT`
  derived) pass through — full layout stays opt-in.
- eqsat repo: **untouched** — the preset is kit-torch's joint-pass
  policy and composes entirely from public `mlopt_core.laws` API, so
  it works on the pinned v0.1.12 with no substrate release. If a
  canonical `laws.JOINT_SAFE` preset is wanted substrate-side later,
  this composition is the spec.

`tests/test_compositional_pairs.py`: +2 tests —
`test_joint_default_is_joint_safe` (no SUBSUMED, no pointwise-layout
names, all 13 structural layout names present) and
`test_joint_rules_widening_scope` (`None` → `_JOINT_DEFAULT`; explicit
`WITH_LAYOUT−SUBSUMED` set passes through by identity).

## End-to-end verification

- Recaptured the deepparallel32 Compositional optimize under the new
  preset: joint `eqsat-run` spec now carries **94 laws** (was 158),
  byte-identical `terms`/`options` to the measured `v81_struct`
  variant; standalone re-run: cost 6.0, extracted term **byte-identical
  to the 158-law result**, 5 420 enodes.
- `pytest tests/test_compositional_pairs.py` — **24 passed** (27.7 s).
- `pytest tests/test_compositional.py` — **7 passed** (40.4 s).
- `pytest tests/test_compositional_cache.py` — **23 passed** (49.2 s;
  the tampered-entry test that took 239 s pre-budgets now fits in the
  file's total).
- `ruff check src` / `ruff format --check src` — clean.
- Note: an earlier combined `pytest` invocation of the last two files
  was OOM-killed (exit 137) under host load ~31; the serial per-file
  reruns above are green.

## Go / no-go

**GO.** Commit on kit-torch branch `joint-safe-preset`; nothing pushed.
Suggested follow-ups (not done): (a) term-aware regime gate — census
the joint IR for `transpose`/NT-GEMM patterns before search and drop
the 13 structural laws too when absent (would recover the residual
2-7× on transpose-free specs; needs a pre-search export or an op
census plumbed into `_joint_rules`); (b) a `LAYOUT` sub-tag
(pointwise vs structural) in the corpus if the split proves stable —
requires kit-eqsat + engine-eqsat regen; (c) canonical
`laws.JOINT_SAFE` preset in eqsat at the next tag if other consumers
want it.

## Artifacts (all /tmp, uncommitted)

- Captured specs: `/tmp/cap_{deep64,tb32,pb32,tt,ts}_01_158laws.json`,
  `/tmp/cap_new_01_94laws.json` (post-change e2e), `/tmp/capA_*`
  (v0.1.9-venv capture — stale budgets, superseded).
- Variants: `/tmp/{spec2,deep64,tb32,pb32,tt,ts}_{full,v81,v81_tp,
  v81_struct,...}.json`; reports `/tmp/rep_*.json`.
- Capture harness: `/tmp/capture_specs.py` (spies on
  `EqsatEngine._invoke`'s `subprocess.run` input).
