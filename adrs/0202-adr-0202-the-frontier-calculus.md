+++
id = "ADR-0202"
title = "ADR-0202: The Frontier Calculus"
status = "open"
priority = "med"
+++

---
title: The Frontier Calculus
type: adr
date: 2026-09-09
status: accepted
status_note: Accepted — frontier terms are data; models may opt out.
---

# ADR-0202: The Frontier Calculus

> Imported from `kit-data-prelude/project` — originally `ADR-0101` (global registry repo `lang`); renumbered `ADR-0202` in the host merge.

## Decision

1. **`Term` gains the frontier constructors** — `CellId`, `CellCompV`,
   `CellCompH`, `Whisker`, `Univalence`, `Glue`, `Fibrant`, `JElim`,
   `Witness`, `Refine`, `Cobind` — as ordinary `Term` variants.
2. **Surface syntax exposes them as data** (the Option B grammar), not as
   engine branches.
3. **Engine semantics**: cells compose vertically and horizontally, whiskers
   attach an arrow to a cell, and `JElim`/`Glue`/`Fibrant` follow the directed
   HoTT reading (ADR-0100).
4. **Serialization is the same as every `Term`** — the constructors are JSON
   data and travel across the boundary unchanged.
5. **Relation to existing ADRs**: this is the concrete term layer for ADR-0100
   (directed limit), ADR-0205 (n-dimensional composition) and ADR-0207
   (generic arrow calculus).
6. **Frontier support is optional**: a model declares whether it interprets
   these terms and may return `Unsupported`; no engine is required to.

## Why

The directed-HoTT reading (ADR-0100) had no concrete terms for cells,
whiskers, univalence or J-elimination, so the theory could not be expressed or
checked. Adding them as data lets models opt in without changing the kernel.

## Not doing
- **New engine primitives per constructor** — they are `Term` data.
- **Require every model to support them** — support is declared.
- **A separate frontier language** — same `Term` space.

## Consequences
- The calculus can express cells and paths; models choose support.
- Serialization and tooling see ordinary terms.
- Engine parity covers only the models that claim support.

## Enforcement
- `crates/handl-kit-data-prelude/` — `Term` variants and grammar rows.
- `data/grammar.yaml` — the Option B surface syntax.
- Model support declarations and serialization tests.

## Design
+The twelve constructors, surface syntax, engine semantics and serialization:
+[`design/0202-frontier-calculus.md`](../design/0202-frontier-calculus.md).
