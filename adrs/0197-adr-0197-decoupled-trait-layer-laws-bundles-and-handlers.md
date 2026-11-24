+++
id = "ADR-0197"
title = "ADR-0197: Decoupled Trait Layer — Laws, Bundles, and Handlers"
status = "open"
priority = "med"
+++

---
title: Decoupled Trait Layer — Laws, Bundles, and Handlers
type: adr
date: 2026-09-02
status: proposed
status_note: |-
  Proposed.  Preserves ADR-0196 §3 (catalog), §4 (storage), §11
  (CutElim), §12 (catalog search as fallback).
supersedes: [ADR-0196]
---


# ADR-0197: Decoupled Trait Layer — Laws, Bundles, and Handlers

> Imported from `kit-data-prelude/project` — originally `ADR-0076` (global registry repo `lang`); renumbered `ADR-0197` in the host merge.

## Decision

1. **Law on an arrow.** A `law` is a standalone declaration that an arrow satisfies an equation. It attaches to any arrow — no bundle required.
2. **Bundle.** A **bundle** is a named record of arrows plus laws over them. It is only needed when a law spans multiple operations that must come from the same impl.
3. **Handler.** A **handler** injects an arrow or bundle at a scope boundary. This is dependency injection over effect handlers — the same machinery as ADR-0052, now the single resolution mechanism.
4. **How they compose.** The three features are independent. No combination is mandatory. The "trait" pattern emerges when all three are present.
5. **`=` in laws.** Inside a `law` context, `=` means extensional equality of arrows (both sides produce the same output for all inputs). This is unambiguous because `law ...` is a distinct syntactic context — the parser knows it is in a law, not in an expression. No special `≡` symbol is needed.
6. **Resolution: handlers primary, catalog fallback.** 1. **Primary: explicit `handle ... with`** — names the impl, lexically scoped, no ambiguity, laws checked at installation. This is the safe default. 2. **Fallback: catalog search** — when no handler is installed, search the open trait catalog (ADR-0196 §3). One match = inject. Multiple matches = error: "ambiguous — N impls match, name one explicitly via `handle ... with`." The compiler never silently picks. 3. **No global coherence** — the problem is avoided by design (lexical scoping), not solved by algorithm.
7. **What this replaces from ADR-0196.** with laws attached). No keyword. installation). No keyword. attachable to any arrow or bundle. context).
8. **What this preserves from ADR-0196.** set. Unchanged. default). `handle` site. Unchanged in mechanism. normalization, non-functor law axioms. Unchanged. demonstrates the catalog + CutElim pipeline. Unchanged.

## Why

ADR-0196 introduced a single bundled construct: `trait` (a named schema of arrow operations plus equational laws), `impl` (a graph assignment for each operation), and `law` (an equation between arrow compositions, declared inside a `trait`). The trait layer bundled eight features:

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- **Simpler.** Three small features replace one large bundled construct.
- **Composable.** The "trait" pattern is an emergent shape, not a
- **No `trait` keyword.** The most complex syntactic category in
- **No coherence problem.** Lexical scoping of handlers eliminates the
- **One mechanism for traits and effects.** Trait resolution and effect

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (mechanism, tables, proofs, trade-offs): [`design/0197-decoupled-trait-layer.md`](../design/0197-decoupled-trait-layer.md).
