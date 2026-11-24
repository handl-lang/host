+++
id = "ADR-0190"
title = "ADR-0190: `vau` as a Typed Escape"
status = "open"
priority = "med"
+++

---
title: '`vau` as a Typed Escape'
type: adr
date: 2026-08-30
status: proposed
status_note: Accepted — `vau` is the one typed escape from structured evaluation.
---

# ADR-0190: `vau` as a Typed Escape

> Imported from `kit-data-prelude/project` — originally `ADR-0045` (global registry repo `lang`); renumbered `ADR-0190` in the host merge.

## Decision

1. **Add `Scheme::Vau { params, body }`**: children are bound as unevaluated
   `AstRef`s, not values, and the body evaluates in the extended scope.
2. **The body may** `eval` a bound expression, inspect the AST, or transform it
   and evaluate the result (metaprogramming).
3. **Add a `vau` keyword and grammar rule** in canonical language data; the
   surface form is data, not an engine branch.
4. **`vau` is the only escape** from structured compositional evaluation; every
   engine implements it consistently (parity tests).
5. **Capability discipline covers it**: a language with `vau` advertises
   `has_vau`/`can_eval`, and a host may deny it (ADR-0046).

## Why

Structured evaluation cannot express every metaprogram, but a host-level
`eval` escape hides the capability. A typed `Vau` scheme makes the escape
explicit in the language data, visible to capability checks, and portable
across engines.

## Not doing
- **A host-only `eval` escape** — invisible to capability policy.
- **Untyped macros** — `vau` is a scheme variant with params.
- **Per-engine `vau` behavior** — parity is required.

## Consequences
- Metaprogramming is expressible and auditable.
- `has_vau` becomes a real capability; policies can deny it.
- Engines must agree on `vau` semantics.

## Enforcement
- `crates/handl-language/` — `Scheme::Vau`.
- `crates/handl-kit-data-canonical/data/grammar.yaml` — the `vau` rule.
- Engine parity tests; `has_vau` in `CapabilitySet`.

## Design
+The scheme variant, syntax, evaluation rules and capability interaction:
+[`design/0190-vau-as-a-typed-escape.md`](../design/0190-vau-as-a-typed-escape.md).
