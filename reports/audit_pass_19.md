---
title: "Audit pass 19 — D-1d landed clean (kit-discovery repo + v0.1.0/v0.1.1 tags verified on remote, graphopt v0.1.19 shrunk consistent); D-1f Phase 1+2 landed (kit-host-tensor v0.1.2, mlopt repinned, forbidden-name renames tagged across four repos, eqsat-pin revert verified correct); deliver v6 paired-round protocol audits sound (one INFO: seeded shuffle can fail to alternate); ext-eval materialize egress closes P18-N1; pin sweep finds only superseded-but-benign tags plus one real corpus skew (eqsat v0.1.9=468 laws vs canonical 524)"
type: report
date: 2026-11-16
---

# Audit pass 19

Scope: commits since pass 18 — kit-discovery repo landing (split
`241875a`, forbidden-name `f495461`, tags `v0.1.0`/`v0.1.1`),
graphopt `4851fd9` (D-1d shrink, tag `v0.1.19`), mlopt `e5362da` +
`27de6ce` (D-1d repin + docs), kit-host-tensor `63d9c12`→`899bf7f`
(D-1f split, tags `v0.1.0`→`v0.1.2`), mlopt `571dd01`/`3994034`
(D-1f repin), forbidden-name rename commits in kit-carriers
(`d545879`/`f7dac24`/`b4545cf`, tag `v0.1.2`), kit-cuda (`870f1f9`,
tag `v0.1.1`), kit-discovery (`f495461`, tag `v0.1.1`), discoverykit
(`08f2392`, tag `v0.1.18`), mlopt `4fa228c` (deliver v6), ext-eval
`eadddba` (ADR-0179 egress, closes P18-N1), plus the pin/tag
consistency sweep and D-1e committed state.

**WIP present and not disturbed:** graphopt's worktree now carries
the **D-1f cascade in flight** — `packages/mlopt-host-tensor/`
staged-deleted (12 files `D `), `pyproject.toml`/`uv.lock` modified
to `kit-host-tensor v0.1.2`, import-linter/ty/coverage blocks
updated, three package-bound test files deleted and three
cross-stack test files modified. Committed state (`v0.1.19` =
`4851fd9`, pushed) is consistent — verified below. mlopt carries
four untracked `arch-{gpt2,t5}-small.{hnl,safetensors}` artifacts.
No `kit-torch` repo exists yet (D-1e Phase 1 is agent-side only).

## Per-commit verdicts

### D-1d landing — CLEAN (resolves P17-N2 / P18-N8)

- **Remote verified, not worktree:** `git ls-remote` on
  `handl-lang/kit-discovery` → `main = f495461`, tags `v0.1.0 =
  241875a` (the `split(discovery)` commit itself) and `v0.1.1 =
  f495461`. Both annotated/lightweight refs pushed; repo is no
  longer empty.
- **graphopt `4851fd9` = tag `v0.1.19`, pushed** (`origin/main` =
  `4851fd9`). `packages/` now holds only `mlopt-torch`,
  `mlopt-host-tensor`, `mlopt-kit`, `mlopt-experiments`,
  `mlopt-handl-peer` — `packages/mlopt-discovery` is gone; `vocab/`
  deleted (−241k lines across 402 files).
- **pyproject consistent:** `[project.dependencies]` and
  `[tool.uv.sources]` both carry `kit-discovery` at `v0.1.0`;
  `uv.lock` has **zero** `mlopt-discovery` entries and locks
  `kit-discovery.git?tag=v0.1.0#241875a` (correct commit).
  `[tool.uv.workspace] members = ["packages/*"]` glob needs no
  edit. `git grep mlopt_discovery HEAD` → only historical
  `docs/reports/*.md` mentions; **no `.py` imports remain**.
- **Coverage/import-linter updated correctly:** coverage
  `source = ["mlopt_torch", "mlopt_discovery"]` →
  `["mlopt_torch"]` (`pyproject.toml:297`) — the discovery floor
  moved to kit-discovery's own `fail_under = 83`
  (`kit-discovery/pyproject.toml:121`, re-measured 2,936 tests).
  import-linter: `mlopt_discovery` removed from `root_packages` and
  the layers table; `kit_discovery` added to the game-layer
  `forbidden_modules` with a comment explaining grimp can't resolve
  an unimported external layer (`pyproject.toml:~195-245`). The
  `kit_cuda` upward edge moved to the forbidden contract — the
  comment correctly notes nothing in the scanned roots reaches it
  post-split.
- **kit-discovery repo self-consistent:** `root_packages =
  ["kit_discovery"]`, forbidden contract lists both `mlopt_*` and
  `kit_*` spellings during the transition; the deliberate lazy
  `kit_cuda` bind (`play/gen.py:657`, `play/torch_arm.py:258`,
  ImportError-guarded) is documented as absent from
  `forbidden_modules` by design (`pyproject.toml:~133-138`) — not a
  gap. `v0.1.0..v0.1.1` shipped-code delta is docstring-only renames
  in `hnl_import.py` + one forbidden-list line — consumers on
  `v0.1.0` lose nothing.
- mlopt `e5362da`: dep + sources + `uv.lock` all moved to
  `kit-discovery v0.1.0`; `27de6ce` docs-only. mlopt `src/` has **no
  stale `mlopt_discovery`/`mlopt_host_tensor`/`mlopt_cuda` imports**
  (grep-verified; all references are `kit_*`).

### D-1f Phase 1+2 — CLEAN; eqsat-pin deviation verdict: **right call**

- **kit-host-tensor** `63d9c12` (subtree split) → `899bf7f`, tags
  `v0.1.0`/`v0.1.1`/`v0.1.2` all on remote, `main = 899bf7f =
  v0.1.2`. mlopt `571dd01` pinned `v0.1.1`, `3994034` bumped to
  `v0.1.2`; `uv.lock` locks `kit-host-tensor.git?tag=v0.1.2#899bf7f`.
  mlopt `src/mlopt/emit.py:1961` `find_spec("kit_host_tensor")` and
  the drift-guard import renamed consistently.
- **Forbidden-name renames landed and tagged:** kit-discovery
  `f495461` (`v0.1.1`), kit-carriers `d545879` (`v0.1.2`, plus
  `f7dac24`/`b4545cf`/`6265f96` gates), kit-cuda `870f1f9`
  (`v0.1.1`), discoverykit `08f2392` (`v0.1.18`). Every shipped-code
  delta across all four is docstring/comment-only — verified via
  `git diff <prev-tag> <latest> -- src/` per repo.
- **The eqsat-pin deviation (`6812b4b`, v0.1.10→v0.1.9): correct.**
  kit-host-tensor `v0.1.0` shipped pinning `mlopt-core v0.1.10` —
  which *would* have conflicted in uv resolution against every
  `v0.1.9`-pinning consumer (dep sources are honored transitively;
  two git tags for one package can't coexist). The revert to `v0.1.9`
  is verified costless for this package: `git diff v0.1.9 v0.1.10
  -- packages/mlopt-core` in eqsat is **13 doc-rename lines across 7
  files, zero functional change** — the v0.1.10 substance (corpus
  468→524 records) lives entirely in `mlopt-orchestrator`'s
  `eqsat_laws.json`, which kit-host-tensor doesn't depend on. See
  P19-N1 for the fleet-level consequence.
- **graphopt-side D-1f is committed-state clean, worktree in
  flight:** HEAD `v0.1.19` still carries `packages/mlopt-host-tensor`
  as a workspace member — consistent. The uncommitted cascade
  (staged package deletion → `kit-host-tensor v0.1.2` git dep,
  import-linter `root_packages`/`source_modules`/ty `extra-paths`/
  coverage comment all updated in the diff) looks mechanically
  correct but is **not yet committed** — same in-flight pattern as
  D-1d was last pass.

### mlopt `4fa228c` — deliver v6 — CLEAN; two INFOs

- **`_bench_paired` (`deliver.py:300-348`) interleaving is correct:**
  each of `rounds` rounds times `iters` forwards of BOTH modules
  back-to-back, within-round order `rng.shuffle`d per round
  (`random.Random(seed)`). Warmup runs once per side at round 0
  (`warmup=warmup if r == 0 else 0`) — both sides warm regardless of
  shuffled order. Score = min over per-round mins = **global min**
  over all timed calls (min is associative — identical statistic to
  the old contiguous protocol); `med_ms` = median of per-round
  medians, symmetric for both sides. `iters` is per-round-per-side;
  `_evaluate` passes `bench_iters // rounds` so total timed calls ≈
  `bench_iters` (exact at the 50/5 default).
- **Seed:** `zlib.crc32(name.encode())` — deterministic per config
  name, different names get different streams. See P19-N4: a fixed
  seed *can* produce a no-alternation sequence.
- **`_eager_for` (`deliver_game.py:192-204`) per-round baseline is
  consistent:** `best_speedup`, `committed_speedup`, `committed()`
  and `_state_pick` (the oracle) all route through it — a config's
  own paired `eager_med_ms` wins when present, construction baseline
  is the fallback. `best_config`/`best_med_ms` still select by
  absolute `med_ms`, which is correct for *shipping* (fastest absolute
  config wins); the paired eager only prices the speedup claim, and
  it's median-vs-median symmetric — no optimistic-bias asymmetry
  (config `min_ms` is reporting-only).
- **MOVE_COST reward:** `reward = committed_speedup − 0.005·steps`
  (`deliver_game.py:320-321`); `committed_speedup` ∈ {1.0} ∪
  (WIN_MARGIN, ∞), steps counts non-stop moves only (stop returns
  before `st.steps += 1` — verified `step()` ordering at
  `deliver_game.py:310-358`). At budget 8 the max penalty is 0.04 —
  learnable, can't go deeply negative. `Report.detail` carries
  `steps`/`move_cost`. ✓
- **Bounded `lowered_by_config` (`deliver_game.py:352-360`)
  invariant holds:** after each step the dict is cleared and
  re-seeded with exactly `st.best_config`'s module. `best_config`
  only picks records with `med_ms` (bench'd configs), and
  `_evaluate` stores `lowered_by_config[name]` before the verify
  gate — so the retained key always exists when `committed()` does
  `ctx.lowered_by_config[best_name]` (`:437`). Verify-failed and
  bench-crashed lowers are evicted — the memory win is real
  (each LowerResult pins weights). No KeyError path found.
- **`oracle_drain` JSON safety:** `_DRAIN_KEYS` (`deliver_train.py:
  155-168`) curates to config/runner_name/min_ms/med_ms/eager_*/
  rounds/bench/verified/unverified/reason — `stats` (the
  potentially non-JSON runner blob) is excluded by construction.
  `rounds` entries are `float()`-cast lists; all retained values are
  str/float/bool/None/list/dict-of-int. `json.dump` default
  `allow_nan` would emit bare `NaN` only if a timing were non-finite
  — `perf_counter` deltas can't be. ✓
- **`imitate_positive_only` indexing verified against gamekit:**
  `engine.imitate_policy` calls `expert(i)` with the enumerate index
  into the passed `cases` iterable (`gamekit/engine.py:163-186`);
  the closure maps `j → _order[j] → picks[original_index]` correctly.
  Pre-draining all cases up front costs one drain per case —
  identical to the old lazy cache whenever `imitate_epochs > 0`
  (default 4); the `imitate_epochs=0` cost increase is documented in
  the docstring. `train()`'s new 5-tuple is consumed by
  `scripts/train_delivery_prior.py:304` and tests — no stale
  4-tuple callers.
- **Tests:** `pytest tests/test_deliver{,_game,_train}.py` → **62
  passed** on the committed tree.

### ext-eval `eadddba` — CLEAN; **closes P18-N1**

- `print_value` (`ext-eval/src/lib.rs:597`) is the single egress for
  `run`/`eval` stdout; both call sites (`:214`, `:261`) updated to
  `Ok(mut value)` + `&mut value` — signature change is complete, no
  other callers exist (grep-verified).
- `handl_value::materialize_tensor_bin(value)` runs before any
  printing — recursion coverage is the whole `serde_json::Value`
  tree (pass-18 verified: objects/arrays all visited, bin records
  nested in `binds` materialize, malformed records untouched).
- Dep wiring: `handl-value = { git = "…/kernel", branch = "main" }`
  added to `[dependencies]`; the existing `[patch."…/kernel"]`
  sibling-checkout entry (`Cargo.toml:81`) covers it. `Cargo.lock`
  gained `handl-value` in ext-eval's deps (+ `libc` transitively).
- New test `print_value_materializes_tensor_bin` builds a real
  4096-element bin record via `tensor_bin_record`, asserts the
  `__tensor_bin__` spelling is gone and `data` is a 4096-list —
  `cargo test print_value_materializes` → **1 passed** (built clean;
  only pre-existing warnings in sibling crates).
- P18-N1 is resolved: `hnl run`/`hnl eval` now emit the array
  spelling, matching hnl-run's contract.

### D-1e kit-torch — committed state only: nothing landed yet

- No `handl-lang/kit-torch` directory exists; no consumer
  (mlopt/kernelopt/graphopt pyprojects, sources, or `src/`) references
  `kit_torch`/`kit-torch` — grep-verified. `mlopt-torch` remains a
  graphopt workspace package at `v0.1.19`.
- `host/project/reports/d1e_kit_torch_prep.md` is the recon (with its
  own correction noting D-1d was still in flight at write time — now
  landed). Phase-1 agent work, if any, is uncommitted elsewhere.

## Pin/tag consistency sweep

Latest tags (all verified = remote HEAD unless noted): kit-discovery
`v0.1.1`, kit-carriers `v0.1.2`, kit-cuda `v0.1.1`, kit-host-tensor
`v0.1.2`, kit-onnx `v0.1.1` (HEAD `1aa20df` one untagged chore ahead),
graphopt `v0.1.19`, eqsat `v0.1.10`, gamekit `v0.1.11` (+benchkit same
tag), discoverykit `v0.1.18`. kit-eqsat and engine-eqsat carry **no
tags** (engine-eqsat deps `handl-kit-eqsat` via `branch = "main"`,
Cargo.lock rev-pins — the Rust-side convention).

| Consumer | eqsat | mlopt-torch | kit-carriers | kit-cuda | gamekit | kit-discovery | kit-host-tensor | kit-onnx | discoverykit |
|---|---|---|---|---|---|---|---|---|---|
| mlopt | v0.1.9 | v0.1.16 | v0.1.1 | v0.1.0 | v0.1.11 | v0.1.0 | **v0.1.2** ✓ | — | v0.1.17 |
| kernelopt | v0.1.9 | v0.1.16 | v0.1.1 | — | v0.1.11 | v0.1.0 | — | — | v0.1.17 |
| graphopt | v0.1.9 | (workspace) | v0.1.1 | v0.1.0 | v0.1.11 | v0.1.0 | v0.1.2 (uncommitted) | v0.1.1 | v0.1.17 |
| kit-discovery | v0.1.9 | — | v0.1.1 | (lazy) | v0.1.11 | — | — | — | v0.1.17 |
| kit-carriers | v0.1.9 | — | — | — | — | — | — | — | — |
| kit-cuda | v0.1.9 | — | — | — | — | — | — | — | — |
| kit-host-tensor | v0.1.9 | — | — | — | — | — | — | — | — |
| kit-onnx | v0.1.9 | — | — | — | — | — | — | — | — |
| discoverykit | v0.1.9 | — | — | — | v0.1.11 | — | — | — | — |

**No pin references a nonexistent tag** — every pinned tag resolves
on its remote. Superseded pins: eqsat `v0.1.9`→`v0.1.10` fleet-wide
(P19-N1, the only one with real content), mlopt-torch `v0.1.16`→
`v0.1.19` for mlopt/kernelopt (P19-N3 — includes a real bugfix), and
the benign rename-only bumps (P19-N2).

## Findings

| ID | Severity | Where | What |
|----|----------|-------|------|
| P19-N1 | LOW | every consumer's eqsat pin (`pyproject.toml` `tag = "v0.1.9"`); `eqsat/packages/mlopt-orchestrator/.../data/eqsat_laws.json` | **Corpus skew**: the whole Python fleet ships the 468-record law snapshot; canonical `kit-eqsat` is at 524 (`engine-eqsat` consumes it via `branch = "main"`). The WP2 corpus growth (488→506→524, incl. the prior_art index-view/zoo ports) is invisible to `mlopt`/`kernelopt`/`graphopt` search until a fleet repin to eqsat `v0.1.10`. Nothing *requires* v0.1.10 (mlopt-core delta is doc-renames only — this is also why the kit-host-tensor revert was free), but Python-side and Rust-side eqsat now run different law banks — extraction results can diverge. Side effect: eqsat@`v0.1.9`'s own `test_eqsat_laws_corpus.py` drift-guards byte-exact against the *live* kit-eqsat sibling checkout, so running eqsat's suite at the old tag fails today. Recommend a deliberate fleet repin commit. |
| P19-N2 | INFO | mlopt/kernelopt/graphopt pins: kit-discovery `v0.1.0`, kit-carriers `v0.1.1`, kit-cuda `v0.1.0`, discoverykit `v0.1.17` | All superseded by `v0.1.1`/`v0.1.2`/`v0.1.1`/`v0.1.18` respectively, but every delta is forbidden-list/docstring/CI-only — zero shipped-code change (verified per-repo `git diff <old> <new> -- src/`). A batch "deps: kit-* repin" commit is cosmetic; safe to defer or fold into the next functional repin. |
| P19-N3 | LOW | mlopt `pyproject.toml:64`, kernelopt `pyproject.toml:56` — `mlopt-torch tag = "v0.1.16"` | Two releases behind `v0.1.19`. Missing `v0.1.17`'s `3e724a8` "creator ops inherit the eval dtype via dtype_ctx" — a real behavioral fix in shipped mlopt-torch code — plus the ADR-0179 and D-1d halves. Verified v0.1.16 has no functional `mlopt_discovery` imports (docstrings only), so nothing breaks; the dtype_ctx fix is worth taking. |
| P19-N4 | INFO | `mlopt/src/mlopt/deliver.py:313` (`rng.shuffle(order)`), seed at `deliver_game.py:637` | The per-round order shuffle is seeded by `crc32(config_name)` — deterministic. Each round's shuffle is an independent fair coin, so ~1/16 of config names get a 5-round sequence that never alternates (all-eager-first or all-config-first), silently degrading the drift defense for those configs. No balance/alternation enforcement or warning. Harmless at CONFIGS≈6 scale; worth a comment or a guaranteed-alternation scheme if the protocol is ever tuned. |
| P19-N5 | INFO | `deliver_game.py:635` + `deliver.py:320` | `bench_iters // rounds` floor + `max(1, …)` clamp: `bench_iters < paired_rounds` yields *more* timed calls than requested (e.g. iters=3, rounds=5 → 5 calls/side), and non-divisible iters under-use the budget slightly (52 → 50). Default 50/5 is exact; CLI `bench_iters` is user-controlled so the edge is reachable but benign. |
| P19-N6 | INFO | `deliver_game.py:647-652` (`rec["rounds"]`) | The persisted per-round pairs record `(eager_min, eager_med)` and `(config_min, config_med)` per round index but **not the within-round execution order** — the audit trail can't verify alternation actually happened (compounds P19-N4's observability gap). |
| P19-N7 | INFO | `mlopt/src/mlopt/weights.py` `DELIVERY_PLAYER_WEIGHTS` | The shipped prior is still the v5 bake — `4fa228c` changed the measurement protocol (paired baselines, MOVE_COST reward, positive-only imitation) without retraining. Expected follow-up per the autopsy plan; flagged so the stale-artifact window is on record. |
| P19-N8 | INFO | kit-onnx HEAD `1aa20df` | One untagged commit ahead of `v0.1.1` (`2784ce8`) — a forbidden-name chore + lock refresh, no shipped-code delta. Consumers on `v0.1.1` current; next tag absorbs it. |
| P19-N9 | INFO | graphopt worktree; mlopt worktree | graphopt carries the uncommitted D-1f cascade (staged `packages/mlopt-host-tensor` deletion → `kit-host-tensor v0.1.2`, pyproject/uv.lock/import-linter/ty/coverage all consistently updated in the diff — mechanically clean, uncommitted). mlopt has 4 untracked `arch-*.{hnl,safetensors}` artifacts. In-flight work, not defects. |
| P19-N10 | INFO | engine-eqsat `dd4815e`, kit-eqsat `04d8064` | Neither Rust repo has tags; engine-eqsat's `handl-kit-eqsat` dep is `branch = "main"` (Cargo.lock rev-pins). The 524-record corpus HEAD is untagged — consistent with the Rust-side convention, noted for the sweep's completeness. |

No HIGH or MEDIUM findings. Both standing MEDIUMs from pass 18 are
resolved: kit-discovery landed with verified remote tags (P17-N2 /
P18-N8 closed), and graphopt `v0.1.18`/`v0.1.19` now exist so the
ADR-0179 halves *can* be pinned (P17-N1 / P18-N7 partially — the
pin bump itself is P19-N3). P18-N1 is closed by ext-eval `eadddba`
with a real test.

**Verdict: PASS** — every committed change in scope audits clean.
The deliver-v6 protocol is carefully built (paired baselines are
symmetric median-vs-median, the retention bound has a verified
invariant, oracle drains are genuinely JSON-safe), the eqsat-pin
revert was the correct call and costless, and the pin sweep found no
broken references — only currency drift, of which the eqsat
468-vs-524 corpus skew (P19-N1) is the one item worth scheduling.
