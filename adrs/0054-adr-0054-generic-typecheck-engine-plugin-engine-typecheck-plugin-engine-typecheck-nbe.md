+++
id = "ADR-0054"
title = "ADR-0054: Generic Typecheck Engine (`plugin-engine-typecheck` + `plugin-engine-typecheck-nbe`)"
status = "open"
priority = "medium"
+++

---
title: Generic Typecheck Engine (`plugin-engine-typecheck` + `plugin-engine-typecheck-nbe`)
type: adr
date: 2026-09-01
updated: 2026-10-29
status: accepted
status_note: Accepted — completes the data-driven typecheck stage; rules are data, one NBE engine ships.
refs: [ADR-0032, ADR-0044, ADR-0052, ADR-0055]
---

# ADR-0054: Generic Typecheck Engine (`plugin-engine-typecheck` + `plugin-engine-typecheck-nbe`)

## Decision

1. **Type rules reuse `Scheme` — `lang.types`.** A rule synthesizes a node's
   type or checks it against an expected one; it may call judgment ops and
   other rules.
2. **Judgment ops are `Arr` ops** (unify, quote, normalize, meta-solve,
   universe `max`, …), so rules compose with the same kernel as `lang.eval`.
3. **`plugin-engine-typecheck` is the abstraction crate** — marker,
   `TypecheckRequest`, `TypecheckEngine`; no engine code, mirroring
   lex/parse/eval.
4. **`plugin-engine-typecheck-nbe` implements `TypecheckEngine`.** NBE —
   `unify`, `quote`/`normalize`, metavariable solving, universe `max` — is the
   implementation behind the trait, not the interface.
5. **Typecheck is wired on `RuntimeHandle` (pure path).** Types are values, so
   checking normalizes through the eval engine and depends on a bound
   `EvalEngine`, symmetric with the other stages.
6. **`plugin-lib-lang-typecheck` is the meta leaf** exposing
   `typecheck.typecheck(lang, ast)`, like the lex/parse/eval leaves.
7. **Rules are authored data** in `crates/handl-kit-data-canonical/data/types.yaml`,
   included into `lang.types` as `eval.yaml` feeds `lang.eval`.

## Why

The typechecker was a Rust `match` engine: every rule was a code arm, so a new
engine or language change meant editing the engine. Types are values (ADR-0032)
and language is data (ADR-0044): rules belong in `types.yaml`, the algorithm
behind a trait. This moves dispatch, not theory.

## Not doing

- **Change the dependent-type theory** — universes, Pi/Sigma/Forall, equality
  and membership are unchanged.
- **Promise a second engine** — the trait makes one possible; one ships.
- **Define the full `types.yaml` rule set** — implementation work; this fixes
  the op set and the seam.
- **Add spread/param destructuring to the AST** — separate AST/grammar/eval work.

## Consequences

- Every type rule is data; a new engine is a trait impl, not a rewrite.
- `types.yaml` becomes the review surface for typing — editable without Rust
  and shared by every engine; typecheck still runs eval to normalize.

## Enforcement

- `crates/plugin-engine-typecheck/` — marker, request, engine trait.
- `crates/plugin-engine-typecheck-nbe/` — the NBE implementation.
- `crates/handl-kit-data-canonical/data/types.yaml` — the authored rules.
- `crates/plugin-lib-lang-typecheck/` — `typecheck.typecheck(lang, ast)`.

## Design

The mechanism, op set, context notes and trade-offs:
[`design/0054-generic-typecheck-engine.md`](../design/0054-generic-typecheck-engine.md).
