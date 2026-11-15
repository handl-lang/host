+++
id = "ADR-0174"
title = "ADR-0174: The end-state architecture — one Term, many interpretations, verified delivery"
status = "accepted"
priority = "high"
+++

---
title: The end-state architecture — one Term, many interpretations, verified delivery
type: adr
date: 2026-11-21
status: accepted
status_note: "Accepted 2026-11-21 — user ratified as the tracking baseline for the whole program. Supersedes no ADR; binds ADR-0172 (Term convergence) and ADR-0173 (kernel extension scope) into one end-state map. Reviewers: this ADR is the checklist — audit the repos against it, not against prose."
applies_to: 'the whole handl-lang/mlopt program — hnl, kernel, engine-*, host, host-tensor, eqsat, graphopt, kernelopt, gamekit, discoverykit, benchkit, mlopt, lib-kernel-ir, compile-*'
---

# ADR-0174: The end-state architecture

## Decision

The program's destination is fixed and tracked here. Every phase,
agent, and audit measures itself against this map.

### The one-sentence version

**A model is lifted once into Handl `Term`s; a search
interpretation optimizes them under verified laws; delivery
interpretations emit runnable artifacts; every claim is measured
and every failure is a named decline.**

### The composition framing (user-ratified 2026-11-21)

**mlopt is a kit, not an engine.** Every source and sink is an
independent repo composed of `engine-*` / `compile-*` / `lib-*` /
`host-*` components — mlopt only *assembles* them (adapters declare
capabilities; `Source`/`Sink` ports bind them; `hnl` is the
composition root). The product repo holds no privileged machinery:
no IR of its own (codec only), no engine, no runtime. If a
component can't be expressed as a kit assembly over existing
engines/hosts/libs, it doesn't belong in mlopt — it belongs in the
substrate repo that owns that layer.

### Layer map (end state)

```
                        hnl — composition root (dumb host)
   hnl mlopt optimize|export|doctor|models · hnl-run · kits
                              │
                    ┌─────────▼──────────┐
                    │  mlopt — product    │  thin composition ONLY:
                    │  adapters·verify·   │  no engine, no IR of its own
                    │  deliver·report     │
                    └──┬───┬───┬───┬─────┘
                       │   │   │   │
              Source ports   │   Sink ports
              (torch·onnx·   │   (torch·host-tensor·
               .hnl·safetensors)  kernel-ir→CUDA·ONNX)
                       │   │   │   │
              ┌────────▼───▼───▼───▼─────────┐
              │  HANDL TERM — THE representation│
              │  arrow calculus · graded dims  │
              │  attrs slot (ADR-0173)         │
              │  __nonfinite__ + dag wire      │
              │  tensor ops = Language fragment│
              └───────────┬───────────────────┘
                          │  interpretations (engines)
        ┌─────────────────┼──────────────────────┐
        ▼        ▼        ▼         ▼            ▼
   exec-*   typecheck  transform  engine-eqsat  compile-*
   (eval)   (NBE)      (rewrite)  (SEARCH —     (native
                                     the game    emit)
                                     changer)
        │                                     │
        └──────────────┬──────────────────────┘
                       ▼
              gamekit — episodes/players/scoring
              discoverykit — law mining → AUTO_TIER
              benchkit — measured ledger
                       │
              kernel-ir → compile-triton → CUDA
              (native path, plan 0039)
```

### Invariants (the honesty contract — never negotiable)

1. **Verify before ship.** The *operative verify verdict* —
   `VerifyReport.passed` composed with task-metric/bound gates
   (`accepted_by` labels which gate decided) — gates search
   extraction, every delivery candidate (game AND heuristic), and
   export (`--verify-run` numeric gate). Missing report = decline.
   *Resolved (eqsat v0.1.2, audit_pass_1):* `Optimizer.lower`
   substrate-enforces the gate — `VerifyDeclinedError` on any
   non-passing/missing operative verdict; consumers name the
   decline. Domain deliver-hooks resolved (audit F2, pass 2):
   `torch_arm.py`/`mlopt_kernel/play.py` now report
   `delivered_unverified` on failed verify — never
   `delivered: True` with `verified: false`.
2. **Declines are named.** Unsupported op, OOM, unverifiable,
   losing config — all explicit, never silent fallback.
3. **Measured or it didn't happen.** Speed claims come from the
   referee (min-vs-min latency), not heuristics; the game is the
   decision procedure, heuristics are the labeled fallback.
4. **One codec.** `.hnl`/Term serialization is the only
   term↔surface translation. *Interim state (arch_review_0174 §4):*
   five Python-side tables exist today — `mlopt.emit._OPS` (199 ops,
   the product emitter), `hnl_surface._SPECS` (12 ops,
   measurement-side), `HOST_BINDINGS`/`torch_bindings` (sink
   lowering), `modelproj`, plus generated `host_surface.json`
   (237 signatures, drift-guarded). The consolidation rule: every
   table must either be generated from `host_surface.json` or carry
   a drift guard against it. `emit._OPS` now carries the
   behavioral drift guard (`test_emit_surface.py`, audit_pass_1);
   `hnl_surface._SPECS`/`modelproj`/`torch_bindings` guarded by
   `test_surface_table_drift.py` and `mlopt_onnx` opmap/liftmap by
   `test_onnx_codec_drift.py` (audit F5, ADR-0178 WP0a). Every known
   table is now generated or guarded. End state: one codec, tables
   retired.
5. **Substrate layering.** The *gamekit kernel modules*
   (`engine`, `scoring`, `calibrate`, `advantage`, `gamedata`,
   `players._base`, `players.registry`) are stdlib-only;
   `players/*` factories and `benchkit` are explicitly
   non-stdlib layers (lazy domain binds by design).
   graphopt domain packages never import game/discovery;
   mlopt is composition only.
6. **Ops are data.** Tensor ops live in a `Language` fragment;
   hosts expose ops; nothing is privileged.

### Phase map (tracked, not committed dates)

| phase | deliverable | status |
|---|---|---|
| P0 codec | `.hnl`↔term codec, `host_surface.json`, `.hnl` Source (`mlopt.adapters.hnl`) | **landed** (graphopt v0.1.4; mlopt Source landed — `adapters/hnl.py` + tests) |
| P0.5 kernel | `Term::Op.attrs` field + `__nonfinite__`/dag wire conventions (ADR-0173) | attrs **landed** (~14 repos); codec envelopes landed graphopt v0.1.5; ADR-0173 proposed pending ratify |
| P1 fragment | tensor `Language` fragment — ops/attrs/guards as data | **landed** (host-tensor `data/language/tensor/ast.yaml`, 238 ops, drift-gated) |
| P2 engine-eqsat | search interpretation over Terms (Rust/native candidate) | **in flight** — `handl-lang/engine-eqsat` scaffolded + first laws; contracts per ADR-0177 |
| P3 migration | mlopt_core typed view demoted/retired; tables → codec | pending P2 |
| P4 differential | Sanada reverse-handler interpretation — verified grads | pending P3 |
| ∥ native | kernel-ir coverage → batched GEMM/attention → compile-triton CUDA | census done; substrate ADR needed |

### Explicit non-goals (do not drift into these)

- No claim that Sanada completeness yields e-graphs — search
  algorithms are built, not inherited.
- No torch-free claim until kernel-ir coverage is measured, not
  asserted (current honest number: 0.12% of lifted nodes).
- No `Ref`/let/vars in the pointless core — sharing is engine
  hash-consing + codec back-references + named leaves (ADR-0173).
- No parallel IR proliferation — `mlopt_core.ir` is the *temporary*
  search IR, not a second permanent representation. The full Term
  census (arch_review_0174): `engine::Term` (arrow calculus — THE
  representation), `egraph::Term` (Rust e-graph node type, already
  `(op, children, attrs)` — internal search structure bridged via
  `term_bridge.rs`, NOT a user-facing IR), `mlopt_core.ir`
  (temporary search IR), `lib_kernel_ir.Term` (native-path IR).
  P2 rule: `engine-eqsat` interprets kernel `Term`s; `egraph::Term`
  stays engine-internal. If a fifth representation appears, it must
  justify itself against this census in its own ADR.

## Why

- The program grew by accretion (monorepo → split → game layer →
  product). Without a fixed map, phases blur and "done" becomes
  vibes. This ADR is the map; reviews audit against it.
- The user's architecture intuition (one IR, many interpretations)
  is the destination; the spike proved the path has a real price
  (kernel attrs field, codec envelopes) — now priced and tracked.
- Honesty invariants are the product's actual moat — encode them
  as reviewable rules, not tribal knowledge.

## Review protocol (for auditing agents)

A reviewer checks the repos against THIS ADR. First audit:
`graphopt/docs/reports/arch_review_0174.md` — its findings are
folded into the invariants above (verify-verdict wording, codec
interim state, Term census, gamekit boundary precision).

1. Does each repo's AGENTS.md/layout match its layer in the map?
2. Do the invariants hold in code (verify gates, decline paths,
   codec singularity, import-linter contracts)?
3. Is any phase claiming done without its deliverable?
4. Is any non-goal being quietly violated (e.g. a fourth op table,
   a privileged VM trait, torch as foundation)?
5. Report: per-invariant PASS/FAIL + evidence (file:line), plus
   drift items — things built that the map didn't anticipate.

## Done means (user-set exit criteria, 2026-11-21)

The saga ends in **audit-and-fix iterations** until the whole is:

- **prod ready** — installs clean, pins coherent, no tribal knowledge
- **documented** — docs site truthful (Diátaxis), ADRs current, AGENTS.md accurate
- **working** — every claimed path runs end-to-end on real models
- **verified** — honesty invariants enforced at substrate, not just callers
- **tested** — suites green, drift guards on every generated artifact
- **useful** — measured wins on real workloads, honest declines elsewhere
- **elegant** — one Term, one codec, layers not names; nothing that can't justify its layer

Each audit cycle (arch_review protocol) produces a fix list; fixes
land as agents; the loop repeats until a full audit finds no
invariant violations, no unguarded tables, no undocumented seams.
"Done" is an audit outcome, not a date.

## Consequences

- New work cites this ADR: "serves P2" or "drift — not on the map."
- Phase completion requires the deliverable + its honesty gates,
  not activity.
- ADR-0172/0173/0175 remain the binding decisions; this ADR is the
  index that keeps them coherent.
