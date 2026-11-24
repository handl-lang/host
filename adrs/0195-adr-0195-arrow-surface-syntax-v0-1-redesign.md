+++
id = "ADR-0195"
title = "ADR-0195: Arrow Surface Syntax v0.1 — Redesign"
status = "open"
priority = "med"
+++

---
title: Arrow Surface Syntax v0.1 — Redesign
type: adr
date: 2026-09-02
status: proposed
status_note: |-
  Proposed

  Establishes the v0.1 surface syntax for the arrow-native language described in
  [ADR-0052 — Arrow Calculus Kernel](./0052-arrow-calculus-kernel.md) and
  [ADR-0069 — Files Are Arrows](./0069-arrow-native-modules.md). Refines the
  combinator surface introduced by ADR-0052 (`arr`/`>>>`/`first`/`choice`/`loop`/`app`)
  into a human-facing syntax, and corrects two design errors carried over from
  the earlier draft: the absence of destructuring renaming, and the misuse of
  `rec` as a recursive-arrow constructor.
---


# ADR-0195: Arrow Surface Syntax v0.1 — Redesign

> Imported from `kit-data-prelude/project` — originally `ADR-0074` (global registry repo `lang`); renumbered `ADR-0195` in the host merge.

## Decision

1. **Correction 1 — Destructuring renaming.** A pattern element may use `source: local` to bind a source field/position to a different local wire name.
2. **Correction 2 — `rec` as a recursive region (Paterson-style).** `rec` **does not define a recursive arrow**. It introduces a **recursive computation region** inside an arrow graph. Bindings inside the region may depend on other bindings in the same region, including bindings textually defined later. The compiler constructs the corresponding feedback graph and checks that the recursive structure is well-typed.
3. **Addendum — Control-flow and construction sugar.** The v0.1 draft specified `if`, `match`, and `rec` as the only control-flow forms. Review against typical TypeScript patterns surfaced seven additional forms that programmers expect and that the calculus already supports via desugaring. They are added here as **surface sugar** — none introduce new kernel combinators (ADR-0052), and none change the type system. Each desugars to `rec`, `match`, `|||`, or structural construction.
4. **What is accepted unchanged from the draft.** The following from the v0.1 draft are accepted as-is:

## Why

ADR-0052 fixed the _kernel_: the calculus is `arr` / `>>>` / `first` / `choice` / `loop` / `app`, and `vau` was removed. ADR-0069 fixed the _module model_: a file is an arrow, imports normalize to projections. Neither addressed the

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- **Destructuring renaming** removes a frequent source of post-destructuring
- **Paterson-style `rec`** keeps the surface syntax aligned with the kernel
- **`rec` regions compose with the rest of the syntax.** A `rec` region is
- **The v0.1 surface is now internally consistent.** The two corrections
- **Optional punctuation** (semicolons, commas) keeps common code compact

## Enforcement

- `lib-option` — where this is true.
- `lib-result` — where this is true.

## Design

The full record (mechanism, tables, proofs, trade-offs): [`design/0195-arrow-syntax-v0_1-surface-redesign.md`](../design/0195-arrow-syntax-v0_1-surface-redesign.md).
