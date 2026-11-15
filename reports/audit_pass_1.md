---
title: ADR-0174 exit-criteria audit — pass 1 (today's shipped surface)
type: report
date: 2026-11-15
---

# ADR-0174 exit-criteria audit — pass 1 (today's shipped surface)

Date: audit of work pushed 2026-11-2x · Read-only; claims spot-checked
against code/tests, no suites re-run. Prior audit:
`graphopt/docs/reports/arch_review_0174.md`.

## Scorecard — "Done means"

| criterion | score | one-line verdict |
|---|---|---|
| prod ready | PARTIAL | engine-eqsat clean; compile-cuda has a `path` dep (not standalone-installable); ADR/checklist docs stale |
| documented | FAIL→PARTIAL | ADR-0174/0173 phase map + gap notes contradict shipped code; kit-mlopt crate docs still call the endpoint "pending" |
| working | PASS | stage endpoint tested end-to-end incl. subprocess + tamper; kit pipeline evals through canonical kit; engine-eqsat/compile-cuda tests exist and are honest |
| verified | BIG IMPROVEMENT | substrate now *enforces* (`VerifyDeclinedError`); residual: domain deliver-hooks bypass `lower`, stage `deliver` artifact never executed |
| tested | PASS | new drift guards (emit↔host_surface, host_surface 238-pin, freshness), stage tamper tests, engine-eqsat/compile-cuda/kit-mlopt suites |
| useful | unchanged | measured-wins story same as prior audit; no regression found |
| elegant | IMPROVING | one codec still far (tables remain) but the biggest table (`emit._OPS`) is now guarded; engine-eqsat honors the Term boundary |

## Invariant verdicts

1. **Verify before ship — PASS at substrate, PARTIAL at edges.** `lower(verify=True)`
   now raises `VerifyDeclinedError` on any non-passing/missing operative
   verdict (`eqsat/.../taskgate.py:346-385`, wired at `optimize.py:1212-1219`).
   This closes arch_review §1's top finding. Product path enforced:
   `mlopt/optimize.py:166-170`, `deliver.py:360-376`, `deliver_game.py:463-489`,
   `stage.py:482,577` all catch/name the decline. **Residual:** the two
   domain deliver arms bypass `lower` entirely (below).
2. **Declines are named — PASS.** Every new path sampled names its reason
   (stage `_StageError`/`_lower_decline_rec`, compile-cuda `as_contract`
   errors, kit-mlopt `EvalError` with child stderr tail).
3. **Measured or it didn't happen — PASS.** Unchanged; bench runners record
   `verified` per row.
4. **One codec — PARTIAL (interim rule now mostly met).** `emit._OPS` gained
   the behavioral drift guard (`mlopt/tests/test_emit_surface.py` — forward
   probe against `host_surface.json` + reverse REGISTRY-decline check).
   `host_surface.json` is 238 ops, freshness-guarded
   (`graphopt/tests/test_host_surface.py:69-71,122-134`). **Still unguarded:**
   `hnl_surface._SPECS` (12-op measurement table), `modelproj`, `torch_bindings`
   — none checked against `host_surface.json`.
5. **Substrate layering — PASS.** engine-eqsat public API speaks kernel
   `Term`s; `handl_egraph::Term` stays inside `pat`/`law` compilation
   (`engine-eqsat/src/lib.rs:13-23`, `pat.rs:1-9`). Minor: `Saturated.graph`
   exposes `handl_egraph::EGraph`/`ClassId` publicly — deliberate
   ("provenance stays inspectable") but the "only `hnl_engine::Term`"
   phrasing is about terms, not types.
6. **Ops are data — PROGRESS.** `Term::Op.attrs` shipped
   (`engine/src/term.rs:79`, `op_native_attrs` et al., serde contract test
   at :1072). `__nonfinite__`/dag wire NOT shipped (zero matches). P0.5 is
   half-landed.

## Prior-audit finding resolution

- arch_review §1 verify gap → **RESOLVED** (substrate raises; consumers updated —
  20+ call sites across graphopt/mlopt catch `VerifyDeclinedError`).
- arch_review §4 `emit._OPS` no drift guard → **RESOLVED**.
- arch_review §4 "five tables" → still five; two now guarded.
- arch_review §1 domain deliver-hooks caveat → **STILL OPEN** (finding F2).
- arch_review drift #1 (egraph::Term vs kernel Term) → ADR-0174 non-goal
  census now names it; engine-eqsat keeps `egraph::Term` internal. Resolved
  in design; watch P3.

## Findings (ranked)

**F1 — HIGH · The audit checklist itself is stale (docs honesty).**
ADR-0174 still carries pre-landing claims:
- invariant 1 "known gap" note (lines 91-94): says substrate enforcement is
  "tracked debt, not an accomplished invariant" — it is now accomplished.
- invariant 4 interim note (lines 104-109): says `emit._OPS` "currently has
  neither" generated-source nor drift guard — it now has the guard.
- phase map P0.5 "proposed" — `Term::Op.attrs` shipped; P2 "pending P1" —
  `engine-eqsat` repo exists (v1 scaffold + tests).
- ADR-0173 status still `proposed`, "Nothing here is implemented yet" (line 23)
  — the attrs field, constructors, and serde contract are implemented;
  `__nonfinite__`/dag wire are not.
Fix: update both ADRs' status/phase tables and fold arch_review_0174's
resolution state into a short addendum, or the next audit re-reports
phantom gaps.

**F2 — HIGH · Domain deliver-hooks still bypass the enforced gate.**
`graphopt/.../play/torch_arm.py:333-345` and `kernelopt/src/mlopt_kernel/play.py:443-452`
call `sink.lower` + `sink.verify` directly and return `"delivered": True`
with `"verified": rep.passed` even when verify fails — the exact pattern
invariant 1 now forbids at `lower`. They are measurement arms (`--deliver`),
but ADR-0174's wording ("every delivery candidate — game AND heuristic") is
broader than "the `Optimizer.lower` path". Either route them through
`lower(verify=True)`/`VerifyDeclinedError`, or scope the invariant text to
shipping paths and mark these records `delivered_unverified`.

**F3 — MEDIUM · `mlopt stage deliver` ships an artifact nothing executes.**
`stage.py:577` re-runs `lower(verify=True)` — good, tamper-proof (test at
`test_stage.py:230-252`). But the shipped `.hnl` artifact is emitted
(`emit_program`, the guarded `_OPS` table) and written with **no execution
check** — the verify verdict covers the torch module/term, not the emitted
program's hnl-run semantics (f64-only, `_DROP_ATTRS`, positional leaf
binding). `delivered: True` + `verified.passed` invites reading "artifact
verified". `export --verify-run` exists as the numeric gate but the stage
doesn't offer it. Recommend a `KitRunner` smoke or an explicit
`artifact_executed: false` field/note.

**F4 — MEDIUM · `_supported_or` residual: ambient bound trusts rule patterns.**
`discoverykit/.../meta_arena.py:788-814,1576-1578`. The synop* hole is
closed — `_prim_definitions()` (`:741-767`) seeds all four record families
incl. `_AUTO_RECORDS` (`minted.py:482-502`), tests pin spelled-parity
billing + `deliverable` resolution (`test_meta_arena_supported.py`). **Residual:**
`supported = ops(term) ∪ ops(DEFAULT lhs/rhs) − prim_defs` assumes every op
a shipped rule names has a real denotation — an unverified invariant. A
law RHS naming an op with no kernel/binding (future minted family outside
`prim_premises`, or a `declare` kernel colliding with a pattern name) bills
at `cost_fn`, wins extraction cheap, and `deliverable` (`:1795-1816`) leaves
it raw → fails only at lowering, after the dishonest price decided.
Also note the asymmetry: explicit `supported=` passes through verbatim
WITHOUT subtracting `defined` (fine if the sink's claim is authoritative),
while ambient subtracts prim_defs even when real torch bindings exist —
conservative direction, but the two paths price the same board differently.
Fix: a census test asserting `board.supported ⊆ (ops with bindings ∪
definitions)` for the shipped DEFAULT.

**F5 — MEDIUM · Unguarded codec tables remain.**
`hnl_surface._SPECS` (12 ops, `hnl_surface.py:140`), `modelproj`, and
`torch_bindings` have no drift guard against `host_surface.json` — the
ADR-0174 interim rule ("every table generated or guarded") is met for
`emit._OPS` + `HOST_BINDINGS` only. These are measurement/sink-side, so
severity is lower, but the rule as written is still violated.

**F6 — LOW · kit-mlopt crate docs stale + `deliver` unreachable.**
`kit-prelude/crates/kit-mlopt/src/lib.rs:26-38` still documents the mlopt
endpoint as "pending … every stage call fails loudly" — `mlopt stage`
landed (`mlopt/stage.py`, `cli.py:425`). The kit exposes only
lift/search/verify (`lib.rs:186-223`); mlopt's `deliver` stage exists but
no pipeline op reaches it — undocumented seam (probably deliberate —
`deliver` needs `out` — but say so). `StageFn::call` (`:173-176`) silently
drops `args[1:]`.

**F7 — LOW · `builtin::reshape_flatten` is a non-equivalence in public API.**
`engine-eqsat/src/builtin.rs:98-114` — reshape→flatten is not
value/shape-preserving in general; it's labeled "demo"/"Not part of
`simplification()`" but is `pub` and usable in `optimize()`. Move to tests
or gate behind an explicit "unsound-demo" name.

**F8 — LOW · compile-cuda pinning.**
`compile-cuda/Cargo.toml:17` — `compile-triton = { path = "../compile-triton" }`
is a path dep; siblings use git+`[patch]` dev-links. A standalone clone
can't build. Also edition 2021 vs siblings' 2024. Honest declines +
structural/nvcc-optional tests are good (`contract_test.rs`,
`README.md:24-53` coverage section is admirably explicit).

**F9 — LOW · `optimize_model` flattens the decline.**
`mlopt/optimize.py:166-170` catches `VerifyDeclinedError` as generic
`Exception` → `report.error` names the type (honest) but `report.verified`
stays `None`, not `False`, and the declined report's `max_abs`/`accepted_by`
are dropped (`VerifyDeclinedError.result` unused). Render shows `verify=None`
for what was a *failed* gate — undersells the decline vs `verify=False`.

**F10 — LOW · measurement-lane soft spots (all disclosed, none blocking).**
- `bench/discovery/runner.py:204-207` + `limits/runner.py:239-242`: declined
  verify → keep timing `e.result.module`, record `verified=False`. Honest,
  but `scorecard.py:160` still prints `beat (measured)` for unverified rows
  — a "beat: true / verified: false" pair could be misread; consider
  suppressing the beat column when `verified is False`.
- `stage.py:274` drift check compares only `ir.root` — inputs/params drift
  invisible (mostly moot: leaves are name-keyed).
- `stage.py:521` `zip(..., strict=False)` truncates silently on arity
  mismatch (downstream emit decline catches unbound leaves).
- `compositional.py:408` — `VerifyDeclinedError` under `verify=verbose`
  now lands in the generic `except` → block status `"failed"` rather than
  the gate's decline taxonomy; outcome identical (keeps original), label
  coarser.
- Protocol can't express `task=`/`task_tol=` contracts — `stage.py`
  `_rebuild_result` doesn't set `SearchResult.task`, and `search` is
  called without `task=` (`stage.py:374`), so the pointwise/bound gate is
  the only operative verdict in the pipeline path. Consistent, undocumented.

## Actions the auditor could not perform

- Run `cargo test`/`pytest`/`lint-imports` — claims verified statically;
  engine-eqsat (`cargo test`), compile-cuda, kit-mlopt, and mlopt
  `test_stage.py`/`test_emit_surface.py` should be run before the fix
  loop relies on them.
- Check `git log` for today's exact commit set — audited by surface area,
  not commit diff.
