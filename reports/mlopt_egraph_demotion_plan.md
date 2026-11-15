---
title: `mlopt_core.egraph` demotion plan — Python e-graph → oracle/edge-fallback
type: report
date: 2026-11-2x
status: plan (P2-finish; binds ADR-0174 phase P2/P3 and ADR-0178 WP4–WP5)
---

# `mlopt_core.egraph` demotion plan

**Scope.** The Python e-graph in `eqsat:packages/mlopt-core`
(`egraph/` ~5k lines + the `laws/` bank it executes) is the *current*
search interpretation. engine-eqsat (Rust, `handl-lang/engine-eqsat`)
is now the proven successor: WP4 parity sweep v3 measured **4/9 cost
parity, 9/9 verified, 0 hard failures** — verified-but-**NO-GO** for a
default flip (`host/project/reports/wp4_parity_sweep_v3.md`). This plan
sequences the demotion from default engine to oracle/edge-fallback per
ADR-0178's D1–D6 and WP4–WP5 gates. It is a plan, not code.

Inputs read: ADR-0174 (end-state map + honesty invariants), ADR-0178
(P3 sequencing, D1–D6), `p3_codec_consolidation_spike.md` (table census
+ consumer map), `wp4_parity_sweep_v3.md` (+ v1/v2 verdicts),
`kit-eqsat` corpus state, and a live import census of the workspace.

> **Status update (post-WP4 v4 GO):** Steps 0–2 landed — the
> 243-record repin (`eqsat 6574c86`), the never-worse-than-input
> floor (`eqsat 5177d97`), and the attr-envelope fix
> (`engine-eqsat 736f810`) produced the **GO** verdict
> (`wp4_parity_sweep_v4.md`: 7/9 cost parity, 9/9 verified, 0 hard
> failures).  **Step 4 (default flip) has landed on eqsat `main`:**
> `_resolve_engine(None)` now resolves to `"eqsat"`;
> `engine="python-egraph"` (aliases `"python"`/`"egraph"`) is the
> explicit opt-out; a missing `eqsat-run` binary stays a named
> `EqsatUnavailableError` (never silent fallback); `detect_*`
> opt-ins on non-`EGraph` engines now fail loudly (`TypeError`,
> same contract as `error_budget`); `discover_alternatives` /
> `Optimizer.discover` pin `"python-egraph"` since frontier
> inspection is proof-carrying `EGraph` machinery.  Downstream
> repos still pin eqsat `v0.1.4` by tag — the flip reaches them at
> the next repin, which is also the Step-3/4 gate: expect
> `engine="python-egraph"` opt-outs in tests pinning pairing /
> carrier-upgrade / certificate / `policy=` semantics.  Step 5
> (soak → demote) and Step 6 (archive decision) remain open.

---

## 1. Current dependency census

### 1a. The engine itself and its bank (eqsat repo)

| component | location | role |
|---|---|---|
| `mlopt_core.egraph` | `eqsat/packages/mlopt-core/src/mlopt_core/egraph/` (`core` 1785, `extract` 1178, `proof` 709, `certs` 279, `terms` 426, `match` 167, `types` 405 lines) | the Python e-graph: saturate, extract, certificates, proof edges |
| `mlopt_core.laws` | `eqsat/.../laws/` (`tensor` 3024, `priorart` 3035, `attention`, `headshare`, `pairing`, `scan`, `specials`, `factored`, `layout`, `cond.py` 2395-line DSL, `ruleset`, `serialize`, `store`, `minted`, `provenance`) | `DEFAULT` = **134 rules**, `ALL_RULES` = **146** + minted store |
| minted store | `eqsat/.../laws/data/minted.db` (sqlite) | tiers 0–4; `AUTO_TIER = 4` currently holds **3 records** (live auto-admissions possible via `admit_prim_record`) |
| `mlopt_native` | `eqsat/packages/mlopt-native` (PyO3/Rust, excluded from uv workspace) | Rust port of *the same Python egraph semantics* — `engine=` opt-in today, **superseded by engine-eqsat** |

### 1b. In-repo consumers (eqsat)

- `mlopt_orchestrator.optimize.search` — the single pipeline entry.
  `_resolve_engine` (`optimize.py:361`) maps `engine="eqsat"` →
  `EqsatEngine`; `engine=None` → the pure-Python `EGraph`. Every
  `Optimizer.search` caller therefore already goes through the
  `ports.Engine` seam — **the swap is an adapter problem, not a
  pipeline rewrite** (ADR-0178 rationale confirmed in code).
- `mlopt_orchestrator.eqsat` (`eqsat.py`, ~700 lines) — the WP3 bridge:
  **CLI subprocess** (`eqsat-run` JSON-in/JSON-out), *not* PyO3.
  Ships `data/eqsat_laws.json` = **111 LAW_FORMAT=2 records**,
  byte-drift-guarded against `handl-lang/kit-eqsat`'s corpus by
  `scripts/emit_eqsat_laws.py` + `tests/test_eqsat_laws_corpus.py`.
  Reports `stats["eqsat"]["laws"]["uncovered"]` verbatim — coverage is
  reported, never silently narrowed (the honesty contract, working).
- Other orchestrator modules importing egraph/laws:
  `bounds.py`, `cache.py`, `compositional.py`, `crossblock_cse.py`,
  `diagram.py`, `lifts.py`, `morphisms/{boundary,graph,reify,signature}`,
  `morphisms_kv.py`, `regime.py`, `__init__.py`.
- `mlopt_core` internals: `cells.py`, `game.py`, `ir.py`, `lawdata.py`,
  `meta.py`, `opdata.py`, `pipeline.py`, `ports.py`, `rulecache.py`,
  `search_env.py`, `trajectories.py`, `typing.py` — the typed view,
  cost algebra, and policy machinery the e-graph consumes.
- `tools/build_minted_store.py` — the store baker.

### 1c. Downstream consumers

**graphopt** (248 census hits — the heaviest consumer):
- Shipped packages: `mlopt-discovery` (~25 modules — `pipeline.propose`
  saturation, `arena`, `enumerate`, `handl_egraph`/`handl_engine`,
  `oracle`, `propose`, `state_reindex`, `vocabdir`, game arms),
  `mlopt-carriers` (`decode_laws`, `om`, `trace`, `xcarrier` — carrier
  law sets compose into `rules=`), `mlopt-torch` (`composer`,
  `models/hybrid`, `torch_bindings` attr-schema overrides).
- Bench: ~60 drivers/suites/probes under `bench/` — the measurement
  surface (policy trainers, overnight loop, parity/differential
  drivers incl. `eqsat_parity_sweep.py`, `eqsat_differential.py`,
  `egraph_differential.py`).
- Tests: the domain-bound suite pins egraph behavior heavily.

**kernelopt** (10 hits): `mlopt_kernel/{board,laws,priorart,cost,sink,
scan,play,weights}` — `KernelBoard` uses `mlopt_core.egraph.verify_certificate`
+ `SearchEnv` for in-graph certificate replay; `KERNEL_RULES`/
`KERNEL_PRIOR_ART_RULES` are its own bank over the same `Rewrite` type.
`mlopt_kernel.search` (ADR-0165) runs over `lib-kernel-ir` Terms — a
*different* search path, unaffected by the demotion except that its
laws/costs share the `mlopt_core` types.

**mlopt product** (1 shipped hit): `src/mlopt/stage.py` — the
lift/search/verify/deliver JSON protocol reaches search through
`Optimizer.search` (engine-agnostic). `deliver.py`/`deliver_game.py`/
`deliver_train.py`/`emit.py` consume *results*, not the engine.

**kit-eqsat**: canonical law corpus — 8 packs, **243 records**
(`simplification` 35, `categorical` 50, `prior_art` 25, `attention` 8,
`pointwise_factor` 2, `batch_norm` 3, `conv_folds` 60, `prim_folds` 60).
`prim_folds.json`'s 60 records = minted.db tiers 1–4 projected
(21+2+34+3) — **the AUTO_TIER→record projection already exists and is
drift-tested**; live `admit_prim_record` writes need a re-projection
step to reach the corpus.

### 1d. Census bottom line

Every shipped consumer reaches the e-graph through either
(a) `Optimizer.search(engine=…)` — already engine-agnostic, or
(b) direct `EGraph`/`Rewrite`/law-bank use in discovery/game/bench
machinery — the *oracle and minting* role, which the plan keeps
Python-side permanently. No consumer hard-codes `EGraph` inside the
product path; the demotion is therefore an **engine-default +
capability-coverage** problem, not a call-site migration.

---

## 2. What moves vs what stays

### Moves to engine-eqsat (Rust) / kit-eqsat (data)

| Python asset | destination | status |
|---|---|---|
| e-graph saturate/extract hot loop | `hnl-engine-eqsat` (`EGraphEngine`) | landed; WP4-verified 9/9 sound |
| Guard-free + cond/dspec-guarded laws | `kit-eqsat/data/laws/*.json`, `LAW_FORMAT=2` | 243 records in corpus; **only 111 reach the wire** (`eqsat_laws.json` is stale — kit-eqsat grew past the last eqsat repin) |
| `cond.py` declarative predicates + shape specs | `guard.rs`/`shape.rs` interpreters | stage-2/3 landed upstream; `tval`/`tapp`/`$leaf:` tensor channels refuse by name (`GuardError::Unsupported`, guard.rs:1172) |
| `count`/`size`/`op_table` cost specs | `cost.rs` `CostSpec` | landed; `flops`/`param_bytes`/`executor_*`/`fusion_*`/`roofline`/`lowering_aware` refuse `CostError::UnknownKind` — named, never approximated |
| minted tiers 1–4 records | `prim_folds.json` pack | projected (60 records); needs a live re-projection hook on `admit_prim_record` |

### Stays Python-side (permanently or until a later phase)

| asset | why it stays |
|---|---|
| `laws/store.py` + `minted.db` + `admit_prim_record` + `provenance.py` + gauntlet | the **discovery/minting machinery** — admission is a Python-side research loop; its *output* projects into `LAW_FORMAT=2` records (kit-eqsat), the store stays the ledger (kit-eqsat AGENTS §ownership) |
| `laws/cond.py` procedural `check=`/`derive=` hooks | concentrated in attention/layout/`conv_bn_fold`; serialize as `"serializable": false` + `missing_hooks` — **named Python-only refusals**, consumers needing them keep the Python engine (honest decline, not approximation) |
| `egraph/proof.py` certificates + `ProofEdge` provenance | ADR-0177 defers certs to P3+; `KernelBoard` certificate replay and `egraph_differential.py` keep needing them — oracle role |
| `policy=` in-search ordering, `truncation_level` bounded-member semantics, causal const-fold opt-out | engine-feature gaps; each is a port-or-decline decision (ADR-0178 D3) — `policy` already declines cleanly for non-EGraph engines (`optimize.py:504` `_policy_kwargs`) |
| `mlopt_core.typing` shape inference beyond stage-3, backend cost profiles | correctly deferred; cost specs arrive as `op_table` data or named declines |
| `mlopt_core.ir` typed view + `term_to_data` | demotes with D5 (Adapters go Term-native); the e-graph consumes it today — out of this plan's scope except as a noted dependency |
| `mlopt_native` | **retire at D6** — superseded by engine-eqsat; do not maintain two Rust search engines (spike §1e) |

### The honest-decline contract at every step

Already implemented and must be preserved through the flip:
- `EqsatUnavailableError` when no `eqsat-run` binary resolves —
  never silent fallback (`eqsat.py:109`).
- `stats["eqsat"]["laws"]["uncovered"]` lists every requested law
  with no portable record.
- `GuardUnsupported`/`Unserializable`/`UnknownKind` refusals in Rust.
- `nonlocal_passes`/`improved` stats honestly marked
  `"skipped (engine is not an EGraph)"` / popped when null
  (`optimize.py:901`, `eqsat.py:617`).
- `VerifyDeclinedError` substrate-enforced on `lower(verify=True)`
  regardless of which engine produced the term.

---

## 3. Sequencing — ordered steps with gates

Follows ADR-0178 WP3→WP4→WP5; D1–D6 naming kept for traceability.

**Step 0 — repin + re-measure (unblocks everything).**
Land the kit-eqsat **243-record** corpus into eqsat (the v3 sweep
measured the stale 111-record wire file) and re-run
`bench/drivers/audit/eqsat_parity_sweep.py`. *Gate:* new sweep JSON.
*Honest-decline check:* if `conv_bn_fold`/`rms_norm_fold`/
`mul_unsq_nat` still don't fire, they are coverage gaps, not bugs —
they stay named-uncovered.

**Step 1 — F3 cost-spec fix (must-fix before any flip).**
`select_mul` regresses 15 → 38 under `{"kind":"size"}` extraction —
a *verified delivery worse than doing nothing*. Fix = map the case's
pipeline cost spec into `CostSpec`/`op_table`, or add a
never-worse-than-input extraction guard. *Gate:* `select_mul` ≥ python
arm on the sweep. This is the only measured *regression*; everything
else is coverage or partial parity.

**Step 2 — WP4 GO/NO-GO decision (D4 gate).**
Re-run sweep; GO requires: cost parity on all paying cases, 0 verify
failures, uncovered set = named procedural-hook tail only. *If NO-GO:*
return to Step 0/1 — the demotion clock does not start on schedule.

**Step 3 — measured parity soak on real models (D4 evidence).**
Beyond the 9-case synthetic sweep: run `engine="eqsat"` vs default on
the real-lift corpus (`bench/suites/` + mlopt adapters — whisper/kokoro
arms) under the bench referee. Record per-case cost/verify deltas.
*Gate:* no verify-gate regression, no worse-than-input deliveries,
bench ledger committed.

**Step 4 — flip default behind flag (D4).**
`engine=None` → `"eqsat"` becomes default *with explicit opt-out*
(`engine="python-egraph"` or the `EGraph` class directly — the port
already accepts instances). Keep `EGraph` importable and default for:
procedural-hook laws (`serializable: false`), certificate consumers,
`policy=`/`truncation_level` users, `mlopt_native`-style features.
The resolver must *name the decline* when a requested rule set is
mostly uncovered (extend the existing `uncovered` stat into a
pre-flight warning, not a silent narrow). *Gate:* full dev-lane suite
+ `scripts/gates.sh` green with eqsat default.

**Step 5 — soak period → demote to explicit opt-in (D4/D6).**
After a measured soak (recommend one release cycle / bench epoch):
rename default to `engine="python-egraph"` explicit-only for the
fallback paths; `mlopt_core.egraph` docstring/status → "oracle +
edge fallback". `mlopt_native` retired (D6) — its semantics are
subsumed; keep the crate archived, not maintained.

**Step 6 — archive/delete decision (D6, gated on ADR-0175 phase).**
`egraph/` proper and un-re-expressed procedural-hook laws either (a)
get ported or named-declined permanently, or (b) move to a frozen
`oracle` extra. Deletion is *not* required for the demotion — the
oracle role is permanent per ADR-0178 ("the differential harness is a
kept test, not a phase").

**Parallel track (not blocking the flip):** `mlopt_core.ir`/D5 —
adapters go Term-native, `term_to_data` retires. The e-graph demotion
*reduces* pressure here (one fewer consumer of the typed view) but
does not gate it.

---

## 4. Blockers — known gaps, must-fix vs accept

| gap | measured state | disposition |
|---|---|---|
| **Coverage: 272/341 requested laws uncovered (80%)** on the 111-record wire | stale snapshot; kit-eqsat has 243 | **must re-measure** (Step 0). Expect residual = procedural-hook/conv tail; each stays a named decline |
| **F3 `select_mul` cost asymmetry** — verified 15→38 worse-than-input | open | **must-fix** (Step 1) — the only true regression |
| **Procedural `check=`/`derive=` hooks** (`conv_bn_fold` named example; attention/layout concentration) | `serializable:false`+`missing_hooks` records exist | **accept as Python-edge** — honest decline; port individually only if a paying workload demands it |
| **`tval`/`tapp`/`$leaf:` tensor channels** | `GuardError::Unsupported` by name (guard.rs:1172, 2774 tests) | **accept as Python-edge** — tensor-valued guards have no kernel-Term analogue; laws needing them keep Python engine |
| **Cost kinds beyond count/size/op_table** (`flops`, `param_bytes`, `executor_*`, `fusion_*`, `roofline`, `lowering_aware`) | `CostError::UnknownKind` refusal | **accept as named declines**; `op_table` + shape inference covers the shipped cost tables — verify per-case in Step 3 soak |
| **`policy=` ordering, `truncation_level`, causal fold, certificates** | `_policy_kwargs` declines for non-EGraph; nonlocal passes skip | **accept** — port-or-decline per ADR-0178 D3; certificates stay Python permanently this phase |
| **`mlopt_native` dual-Rust-engine** | opt-in, same semantics as Python egraph | **retire at D6** — not a blocker, a cleanup |
| Partial parity: `rms_norm_fold` 218 vs 129, `swiglu_mlp` off-by-one | fires helpers, not the fold | **must re-measure post-repin**; if still partial → coverage item, not blocker |

---

## 5. Risks

1. **minted.db / AUTO_TIER coupling.** `admit_prim_record` mutates the
   shipped store *live*; `MACHINE_SHIPPED`/`AUTO_ADMITTED` fold new
   names into `DEFAULT` at import. The kit-eqsat projection
   (`prim_folds.json`) is a *snapshot* — a live admission between
   projections silently widens the Python bank while the eqsat wire
   corpus lags. **Mitigation:** make `emit_eqsat_laws.py` (or an
   `admit_prim_record` post-hook) re-project on admission, and add a
   drift test asserting `AUTO_TIER ⊆ projected records`. Without this,
   the flip creates a two-speed law bank — the worst kind of drift.
2. **Benchmark comparability.** `bench/` ledgers pin `stats["engine"]`
   and rule sets; flipping the default changes what every historical
   baseline *means*. **Mitigation:** ledger rows must record engine +
   law-corpus version (the `eqsat` stats block already carries
   requested/applied/uncovered — extend with corpus hash); new-baseline
   epoch declared at Step 4, old rows never reinterpreted.
3. **`improved`-stat / null-guard semantics.** `EGraph.run` records
   `improved` only under `stop="improving"`; the bridge pops the key
   when null (`eqsat.py:617`) and `nonlocal_passes` reports
   `"skipped (engine is not an EGraph)"`. Consumers diffing stats
   across engines see *structurally different* dicts — a silent
   semantic break for policy trainers reading `stats["improved"]`.
   **Mitigation:** normalize stats at the `ports.Engine` boundary
   before the flip; audit `bench/drivers/policy/*` consumers.
4. **Verify-gate is the real safety net — keep it loud.** Every
   eqsat delivery still passes `lower(verify=True)`; the F3
   worse-than-input case *verified* — soundness ≠ quality. The flip
   gate must include measured-cost regression checks (Step 3), not
   just verify pass rates.
5. **Corpus-version skew.** `eqsat_laws.json` (111) vs kit-eqsat
   (243) vs `DEFAULT` (134+AUTO) is *three* law-set versions in
   flight. Each repin without a sweep re-run repeats the v2→v3
   stale-measurement mistake. **Mitigation:** one command
   (`emit_eqsat_laws.py --check` + sweep) as the corpus-release gate.
6. **Subprocess bridge latency/progress.** `EqsatEngine` is a
   `eqsat-run` subprocess per `run()` — fine for one-shot `search`,
   but game/discovery loops calling `run` per move pay process-spawn
   per episode step. `mlopt_discovery` arms that saturate per-move
   may need to stay on the Python engine for throughput even after
   the product default flips — **accept as edge**, but measure in
   Step 3 before assuming.

---

## Recommended first concrete step

**Step 0+1 combined:** repin eqsat to the 243-record kit-eqsat corpus
(`emit_eqsat_laws.py` + drift test), rebuild `eqsat-run` at engine-eqsat
HEAD, re-run `eqsat_parity_sweep.py` — *and* land the F3
never-worse-than-input guard in the same sweep iteration, so the next
verdict is GO/NO-GO on the real corpus rather than the stale 111.

Everything else in this plan is gated on that measurement.
