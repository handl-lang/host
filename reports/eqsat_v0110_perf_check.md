# eqsat v0.1.6→v0.1.10 perf check — the "minutes per eqsat-run" regression

Date: 2026-11-16. Investigator: subagent (read-only + benchmarks).
Repos: eqsat `/home/luis/GitHub/handl-lang/eqsat`, engine-eqsat
`/home/luis/GitHub/handl-lang/engine-eqsat`, graphopt
`/home/luis/GitHub/handl-lang/graphopt` (venv actually on eqsat
**v0.1.9**, see §5). Prior report: `graphopt_gates_v0121.md`.

## Verdict

**REAL wall-time regression — but the mechanism is narrower and more
specific than "corpus growth slows every search":**

- Per-block `search()` calls are **unchanged** — same 74 laws on the
  wire, byte-identical records, ~0 s saturation on the measured block.
- The blowup is confined to the **Compositional/Composer
  `cross_pairs` joint pass**, which deliberately widens the rule set
  to `WITH_LAYOUT − SUBSUMED` (219 rules). The eqsat corpus growth
  between v0.1.6 and v0.1.9/v0.1.10 added LAW_FORMAT=2 records for
  **77 layout-pack laws** that this widened set requests — taking the
  joint search's wire set from **81 → 158 laws** (~2×). On a real
  captured joint spec that is **0.35 s → 132 s** of saturation
  (~375×) for an **identical extraction cost**.

So: bigger *applicable* corpus = more work by design, but the
magnitude is pathological — guard `check` evaluations dominate
(121 s of 132 s) and the new layout laws fire essentially unbounded
(only 4 of 83 layout-ish laws on the wire carry a `rule_budgets`
entry). **Needs mitigation**, not just re-marking tests `slow`
(though that is the cheap immediate fix).

## 1. Is search time actually up? — measured A/B on one binary

The corpus is **not embedded** in `eqsat-run`; `mlopt_orchestrator.eqsat`
sends the selected `LAW_FORMAT=2` records on stdin per invocation
(`engine-eqsat/src/bin/eqsat-run.rs:206-234`). So corpus-version effects
are fully isolable with a single binary.

Captured real specs by running
`Optimizer(backend=TorchBackend()).optimize(Stack(2), x,
strategy=Compositional())` (the same shape as
`tests/test_compositional_cache.py::_stack`) with a spy on
`EqsatEngine._invoke`:

| spec | what | nodes | laws on wire | result |
|---|---|---|---|---|
| spec1 | per-block search (DeepParallel, `rules=None`→`default_rules()`) | 9 | 74 | saturate **9.5 ms**, whole optimize 1.85 s |
| spec2 | **joint pair pass** (`cross_pairs`, `_joint_rules`→`WITH_LAYOUT−SUBSUMED`) | 16 | 158 | saturate **131.9 s** (EQSAT_TIMING run; 82 s in an earlier uncontended-ish run), 9 iters, 8 930 enodes, stop=FixedPoint |

Same `eqsat-run` binary (`target/release`, engine-eqsat `dd4815e`),
same term — only the `laws` array differs:

| spec2 variant | laws | wall | iters | enodes | cost |
|---|---|---|---|---|---|
| v0.1.6-applicable subset | 81 | **0.35 s** | 7 | 1 374 | **6.0** |
| current (v0.1.9/v0.1.10 applied) | 158 | **82–132 s** | 9 | 8 930 | **6.0** |
| all 524 corpus records (worst case, never sent by the bridge) | 524 | **>7 min CPU**, killed | — | — | — |

Extraction **cost is identical (6.0)** on the measured term — the extra
77 laws bought zero measured gain here while costing ~375× the time.

Test-level confirmation: `test_tampered_entry_declines_and_recovers`
(**PASSED in 239 s** under today's residual contention — an
`hnl-run` llama verify at 87 % CPU + another agent's coverage suite
on an 11 GB host). It performs 3 `optimize()` calls ≈ 3 joint-pair
searches. During the gates run (heavier contention: concurrent full
pytest + vitest storm + two earlyoom SIGKILLs) the same searches were
observed at 1–5 min each → >25 min for the test. Contention amplifies
but does not cause the regression: the A/B above is contention-
normalized (same binary, back-to-back).

## 2. Where the time goes

`EQSAT_TIMING=1` profile of the 158-law joint spec (132 s total):

```
compile_rules   1.6 ms   (158 laws)
add_json        0.1 ms   (16 -> 16 enodes/classes)
saturate_roots  131 899 ms -> enodes=8930 classes=138 iters=9 FixedPoint
extract_best      2.2 ms
final profile: substs=2 168 544  unions=2 094 176  cand=33 263
               enum=1784ms  resolve=1176ms  check=121 253ms  derive=2045ms
               inst=3117ms  union=603ms
```

- **`check` = 121.3 s (92 %)** — per-candidate guard evaluation
  (shape inference through `guard.rs`/`shape.rs`), not e-match
  enumeration (`enum` = 1.8 s). 2.17 M substitutions → ~2 M guard
  checks; the identity-keyed shape memo (`3443774`, the gpt2
  40 min→29 ms fix) is hitting poorly here — candidates differ per
  substitution, so memo keys miss.
- E-graph growth: 16 → 152 → 888 → 2 191 → 4 049 → 8 930 enodes over
  9 iterations — the layout laws keep the frontier dirty.
- Top merge-attributed fires: `comm_add` 2 352, `transpose_pull_add`
  1 342, `linear_from_matmul_t` 1 317, `assoc_matmul_rev` 384,
  `distribute_matmul_over_add` 384, `weight_distribute_matmul` 340 …
- Of the 8 403 attributed merges, **3 161 come from the 6 new-layout
  laws that fire**: `transpose_pull_add` 1 342, `linear_from_matmul_t`
  1 317, `transpose_push_add` 248, `matmul_transpose_rev_dd` 98,
  `transpose_matmul` 86, `linear_to_matmul_t` 70.
- `budget_suspended`: 6 rules hit their 512 `rule_budgets` cap
  (`assoc_add`, `assoc_matmul_rev`, `distribute_matmul_over_add`,
  `factor_matmul`, `right_distribute_matmul`,
  `weight_factor_matmul`) — the budget machinery works, **but the
  layout laws are not in `rule_budgets` at all**: 83 layout-ish laws
  on the wire, only 4 budgeted → effectively unbounded firing.

## 3. Real regression or expected cost?

**Real regression in wall-time; not a correctness regression.**

- The wire law set for joint searches genuinely grew (81→158) because
  the corpus now *covers* laws the Python `WITH_LAYOUT` ruleset always
  requested — before the layout pack landed, those names were
  `uncovered` and silently skipped. More applicable laws ⇒ larger
  e-graphs ⇒ superlinear guard-check cost. That part is "expected".
- But 375× for identical extracted cost on a 16-node term is beyond
  "expected cost": the layout migration laws are *designed* to move
  transposes through matmul/linear chains — exactly the op mix every
  NN block has — and they have no fire budgets, so they explore the
  full transpose-placement space until `max_nodes`/fixed-point.
- Quality side: extraction can only improve or tie with a larger law
  set (min over a superset), and `verify=True` gates delivery — so
  this is a time-cost, not a wrong-answer, regression.

## 4. Corrections to `graphopt_gates_v0121.md`

- **The gates venv was on eqsat v0.1.9, not v0.1.10.**
  `.venv/.../mlopt_*-0.1.0.dev0.dist-info/direct_url.json` records
  `requested_revision: v0.1.9`, commit `414ec4e` (dist-info mtime
  04:21; `uv.lock`/`pyproject.toml` repin to v0.1.10 landed 13:36 via
  `389e3f2` — the venv was never re-synced). Moot for this bug: the
  joint applied set is **identical at v0.1.9 and v0.1.10** (218
  records either way — the v0.1.10 growth is all in packs the
  rulesets never request).
- Corpus sizes: v0.1.6 = **243** records (the "301" in the report was
  an intermediate snapshot, not a tag), v0.1.9 = 468, v0.1.10 = 524.
- "301→524 ⇒ every search slower" is wrong: `laws.DEFAULT` is
  **134 rules at every tag** and `DEFAULT ∩ corpus` = 133 with
  **byte-identical records** for all applied names across
  v0.1.6/v0.1.9/v0.1.10 (verified programmatically — 0 record diffs).
  The corpus growth is entirely in opt-in packs (`prior_art`,
  `layout`, `scan`, `conv_folds`, `attention`, …) that only reach the
  wire through widened rulesets — today that is exactly one place:
  `cross_pairs`.

## 5. Where the delta actually comes from (numbers)

`requested` rule names vs corpus records, per eqsat tag
(core `DEFAULT`; joint = `WITH_LAYOUT − SUBSUMED`; synop-mentioning
records excluded as the bridge withholds them unseated):

| tag | corpus | DEFAULT∩corpus | joint∩corpus | joint applied (no-seat) |
|---|---|---|---|---|
| v0.1.6 | 243 | 133 | 141 | **81** |
| v0.1.9 | 468 | 133 | 218 | **158** |
| v0.1.10 | 524 | 133 | 218 | **158** |

The 77 newly-covered joint laws are **all `tags=['layout']`**
(70 `transpose_*`, 4 `matmul_transpose_*`, 3 `linear_*`) — the
layout-migration pack ported into kit-eqsat/engine-eqsat on
2026-11-16 (`eb33eb6`, `d9b7a41`, `60d03ad`, `d777a84` engine-side;
eqsat-side `0e895c9`/`a3ec9bd`/… corpus regens between v0.1.8→v0.1.9).

Call path: `Compositional` strategy →
`mlopt_orchestrator.compositional` (`max_cross_pairs=8` default) →
`mlopt_torch.composer.TorchComposer.cross_pairs` →
`_joint_rules(rules)` widens `None`/default to `_JOINT_DEFAULT` =
`WITH_LAYOUT − SUBSUMED` (`composer.py:41-67`) → `_joint_optimize`
→ `search(rules=…)` → `EqsatEngine.run` → 158 records on the wire.
Per-block searches keep `default_rules()` (74 applied) and are
unaffected. Any caller passing `rules="all+layout"`/`FULL`-ish sets
to a plain `search()` hits the same wall — the joint pass just makes
it the default path for multi-block Compositional models.

## 6. Mitigations (ordered by leverage)

1. **Immediate:** re-mark the worst Compositional tests `slow`
   (`test_compositional_cache.py` last tests, `test_compositional_pairs.py`)
   — matches the gates report's own suggestion; restores the dev lane.
2. **Joint-pass rule budget:** give the layout laws `rule_budgets`
   entries (they currently have none — 79/83 layout-ish wire laws are
   unbounded) and/or add `tags.LAYOUT`-style exclusion to
   `_JOINT_DEFAULT`, or drop SYMMETRY from the joint set too
   (`comm_add` alone fired 2 352 merges under a 512 budget that
   still let the e-graph reach 8 930 enodes).
3. **Engine-side (engine-eqsat):** the cost is 92 % guard `check`,
   i.e. shape-inference re-evaluation per substitution. The
   identity-keyed memo misses because each subst yields a fresh term
   identity — consider keying on (op, attr-tuple, child-shape-tuple)
   or memoising per-e-class rather than per-term; also consider a
   per-rule candidate cap before `check` runs (cheap pre-filter).
4. **Corpus tiering:** the layout pack is opt-in data — a
   `rules=`-driven subset selector already exists
   (`EqsatEngine(laws=…)`/`run(rules)`); the gap is that
   `_JOINT_DEFAULT` requests names whose records now exist. A
   "joint-safe" preset (WITH_LAYOUT minus layout-tagged rules, or
   layout laws with budgets) would restore the v0.1.6 effective set
   without touching the corpus.
5. **Not recommended:** raising `MLOPT_EQSAT_TIMEOUT` (300 s) — the
   132 s search already sits near it; under contention it exceeds it
   and the bridge raises `EqsatError`, which the Compositional pass
   records as a fallback (correct but slow).

## Appendix — reproduction

```sh
# capture a real joint spec (spy on EqsatEngine._invoke), then:
EQSAT_TIMING=1 nice -n 19 \
  ~/GitHub/handl-lang/engine-eqsat/target/release/eqsat-run \
  < /tmp/tamper_spec_2.json > rep.json
# variants built at /tmp/spec2_v016laws.json (81 laws, 0.35 s),
# /tmp/spec2_all524.json (>7 min, killed), /tmp/spec2_74laws.json.
```

Artifacts left in /tmp (not committed): `tamper_spec_{1,2}.json`,
`spec2_{v016laws,all524,74laws}.json`, `rep_*.json`, `timing_t2.txt`,
`eqsat-versions/{v0.1.6,v0.1.9,v0.1.10}/` (git-archive extracts).
