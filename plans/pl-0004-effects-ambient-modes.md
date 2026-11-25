---
title: PL-0004 — Effects as ambient modes (MTT completion)
type: plan
 date: 2026-11-25
 refs: [pl-0002, pl-0003, ADR-0153, ADR-0158, ADR-0203, ADR-0207, design/effects-ambient-modes.md]
 status: approved-direction
---

# PL-0004 — Effects as ambient modes

Goal: complete the MTT thesis — effects become a mode algebra on the
same axis as grades/coeffects, with the judgement carrying an ambient
mode `μ`. `IO<sig>` survives as reification throughout; nothing breaks
mid-migration.

Design: `design/effects-ambient-modes.md`. Supersedes the M4
deferral in `effects-as-modes.md` (blockers are sequenced, not
avoided).

## Dependencies

- **M2** (mode algebra interface, `915066aa`) — `mode_compose`,
  `Algebra` type, call-head instances, canonical default algebra.
- **`new` declaration syntax** (`b2ef7ae0`) — `new Algebra(Row)` for
  the effects algebra; `new Constraint` (instance-unified-registry)
  shares the desugar path.
- **M3.5** (mode polymorphism) — needed for E3.
- **I-005** (model parity: `synth_handle`/`synth_perform` into
  `typecheck_model.hnl`) — prerequisite for E4, worth pulling earlier.

## Phases

### E0 — effects algebra instance (ships inside M2)

- `Effects := new Algebra(Row)` canonical instance:
  `compose` = row union w/ entry-merge rule, `unit` = `{}`,
  `order` = row-width subsumption.
- `~[{io: S}]~>` permitted as annotation; `synth_graded_arrow`
  cross-checks `io`-headed mode vs `IO`-headed codomain (one row
  comparison). Zero semantic change to `IO<sig>`.

### E1 — mode contexts + `@ μ` surface

- Judgement form `Γ ⊢ t : A @ μ` plumbed through NBE synth/check.
- `@ μ` parses/elaborates; `IO<S>` reifies to ambient `{io:S}`
  (bidirectional: old spelling = new mode).
- Mode metavariables: honest bound (unresolvable → pass), matching
  M3 posture.

### E2 — perform extension + handle modality

- `perform(op, arg)` requires `op ∈ μ`, extends ambient mode,
  returns plain `T` (no `IO{op:T}` mint).
- `handle m with h` = modality `μ → μ′`: port `handle_sig_check`
  coverage/residual behind the modality judgment; deep-handler
  codomain rewrite becomes mode-slot rewrite.
- Composition computes rows via `mode_compose` (the demo:
  `{puts} ⋄ {fs} = {puts,fs}`).

### E3 — effects polymorphism

- `runAll`-style mode-polymorphic combinators over ambient modes.
- Row-poly sigs (`IO(s)`) as mode metavariables; solver upgrade path.

### E4 — model parity

- Port E1–E3 into `typecheck_model.hnl`; the `.hnl` model must cover
  what NBE checks (thin-adapter contract). I-005 folds in here.

## Exit criteria

- `handle`/`perform` check identically under both spellings
  (`IO<S>` codomain and `@ {io:S}` ambient) — parity tests.
- Effect rows compose via `mode_compose`; widening via `order`.
- Coverage/residual diagnostics preserved (same three classes).
- Zero regression on the `IO<sig>` corpus; model parity green.

## Risks

- Mode contexts touch every judgment — largest blast radius in the
  plan; E1 lands behind the existing checker with `μ` defaulting to
  the pure/empty mode so un-annotated code is unaffected.
- `perform` mid-expression mode extension needs evaluation-order
  threading — prototype early in E2.
- Row metas → mode metavars is the known-hard corner; honest bound
  is the fallback, not a blocker.
