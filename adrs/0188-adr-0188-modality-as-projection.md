+++
id = "ADR-0188"
title = "ADR-0188: Modality as Projection"
status = "open"
priority = "med"
+++

---
title: Modality as Projection
type: adr
date: 2026-08-24
status: proposed
status_note: Accepted — six moods over one `project(x, K)` primitive.
---

# ADR-0188: Modality as Projection

> Imported from `kit-data-prelude/project` — originally `ADR-0026` (global registry repo `lang`); renumbered `ADR-0188` in the host merge.

## Decision

1. **A modality system built on one primitive**: `project(x, K)` extracts the
   part of `x : S` that inhabits `K ⊆ S`.
2. **Six moods are uses of that projection**: indicative (strict), potential
   (soft → `null`), negative (complement → `null`), interrogative (`bool`),
   normative (`Result`) and conditional (default).
3. **`x not K` is complement projection, not Boolean negation**; Boolean
   negation is `not (x is K)`.
4. **Each mood is a module** (`mood.ind`, `mood.pot`, `mood.neg`, `mood.int`,
   `mood.norm`, `mood.cnd`) exposed through its sugar.
5. **Strict moods error on non-membership**; soft moods return `null`, the
   default, or a `Result`.
6. **Projection composes with grades and sorts** — a mood is not a separate
   type system.

## Why

Mood/modal operators were becoming separate constructs per use. One projection
primitive plus six thin moods keeps the semantics small and makes the
complement/negation distinction explicit.

## Not doing
- **Boolean negation as `not K`** — it is a complement projection.
- **A mood per new use** — the six are exhaustive uses of projection.
- **Separate modal types** — moods are grades over sorts.

## Consequences
- One primitive; moods are sugar/modules.
- Strict/soft behavior is explicit per mood.
- Complement and negation cannot be confused.

## Enforcement
- `crates/handl-kit-data-prelude/` — projection and mood modules.
- Grade/sort tests for each mood.
- The complement-projection examples as tests.

## Design
+The mood table, projection semantics, complement examples and negation:
+[`design/0188-modality-as-projection.md`](../design/0188-modality-as-projection.md).
