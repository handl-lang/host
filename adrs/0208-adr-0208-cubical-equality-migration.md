+++
id = "ADR-0208"
title = "ADR-0208: Cubical Equality — Interval, Glue, and Computing Univalence"
status = "accepted"
priority = "high"
+++

---
title: Cubical Equality — Interval, Glue, and Computing Univalence
type: adr
date: 2026-11-24
status: accepted
status_note: |-
  Accepted. Supersedes the intensional-MLTT reading of `Path`/`J`/`#`
  (ADR-0120 equality fragment) as the *foundation*; the surface syntax is
  retained. Implementation plan: `CUBICAL-PLAN.md` (workspace root).
---

# ADR-0208: Cubical Equality

## Context

Handl's equality fragment is intensional MLTT: `Path(a,b)` is a
`{frm,to}` witness record, `p # v` transport is type-level substitution,
and `j` is a primitive eliminator (`synth_j`/`synth_transport` in
`typecheck_model.hnl`). Univalence, glue, and fibrancy exist only as dead
surface forms (`grammar.yaml`/`tokens.yaml`/`ast.yaml`, no type rules).

Two foundations cannot coexist computationally: intensional MLTT has `J`
but univalence is a non-computing axiom; cubical TT has computing
univalence via `Glue`/`transp`/`hcomp` but no `J` primitive. The reserved
cubical keywords and the project's "everything is inspectable data that
computes" ethos decide the destination.

## Decision

Migrate the equality layer to cubical type theory (CCHM-style):

1. **Interval `I`**: new sort with endpoints `i0`/`i1` and dimension
   variables; face constraints `Φ` (`i=0`, `i=1`, `~i`, `i∨j`, `i∧j`).
2. **`Path A x y := (i:I) → A`** (homogeneous); **`PathP A x y :=
   (i:I) → A i`** (heterogeneous, `A : I → Type`). `refl x := λi. x`.
   The existing `type.pi`/`body_closure` machinery already implements
   `(i:I) → A i`.
3. **`transp`/`hcomp`/`comp`**: transport and composition on open boxes
   with face constraints — replaces substitution-based transport for
   dependent transport. `p # v` is re-sugared where possible.
4. **`Glue A φ (T̂,ê)`**, `unglue`, `isFibrant`: the `glue`/`fibrant`
   surface forms get real type formers and judgments.
5. **Univalence computes**: `ua : (A ≃ B) → Path A B` defined through
   `Glue`; `transp (ua e) ≡ e`.
6. **`J` is removed** as a primitive (or demoted to a derived eliminator
   in a later phase). Dependent `match ... return (i,w) ~> T with`
   keeps its `eq_*` witness refinement, re-grounded on `PathP`/`transp`.

## Consequences

- Types are erased at runtime (ADR-0151), so `p # v` still evaluates to
  `v`; the interval lives entirely in the type theory
  (`typecheck_model.hnl` + YAML fragments). No evaluator changes for the
  core migration.
- `Path{frm,to}` witness records, `subst_index_`/`transport_term_`
  substitution transport, `resolve_path_`/`check_path_intro_`,
  `synth_transport_`/`synth_j_`, and the `Path(a,b)` call branch in
  `synth_type_impl_` are the migration surface (see the touchpoint map
  produced during planning).
- Fixture drift: `typecheck_model.hnl` exists in three copies
  (canonical + two test fixtures) with no sync mechanism; the migration
  must re-sync them.
- Phases land independently stable (P1 interval+Path → P2 transp/hcomp →
  P3 Glue → P4 univalence → P5 elimination rework → P6 docs). Details in
  `CUBICAL-PLAN.md`.
