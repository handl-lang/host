---
title: One Language, Multiple Interpreters — A Graded Arrow Calculus
type: adr
date: 2026-09-08
status: proposed
status_note: Proposed — one graded arrow language; interpreters implement `Model`.
---

# ADR-0098: One Language, Multiple Interpreters — A Graded Arrow Calculus

## Decision

1. **Everything is an arrow, and the arrow language is graded.** Grades are
   the single mechanism for effects, dimensions and refinement.
2. **The graded-theory map is the grade vocabulary** — each grade names its
   theory; no ad-hoc annotations.
3. **`plugin-engine` is the base `Term` language plus the `Model` trait** that
   every interpreter implements.
4. **All primitives and combinators are optional per runtime**; a model
   declares what it supports, so no interpreter carries dead surface.
5. **The arrow-handler algebra is data-driven** and interprets `Term`; handlers
   are composed, not baked into the engine.
6. **`Scheme`/`Term` is the calculus**, with a fixed shape; interpreters
   consume it, they do not extend it.
7. **Surface arrow combinators are `Term` sugar**; bounded repetition is a
   combinator, not a primitive.
8. **Sorts are grade records**; the `data` keyword is removed and inductive
   sorts are ordinary `sort` values.
9. **Generics are `Generic` tagged applications over grades**, not a separate
   type-class layer.
10. **Effects stay structural and handlers composable**: an `op` declares its
    sorts and grade in `Language` data; a model supplies the handlers.

## Why

One language with several interpreters forces the semantics out of the
interpreter: if a construct means anything, it must be in `Term` + grades +
handlers, not in a Rust arm of one engine. The previous split produced a
language per engine and made equivalences unprovable.

## Not doing
- **A language per interpreter** — the semantics would diverge.
- **Special-case `data`/generics** — both are `sort`/`Generic` over grades.
- **Type classes or interfaces** — grades and `Generic` cover the same ground.
- **Ungraded primitives** — every effect carries a grade or the calculus is
  impure.

## Consequences
- Engines shrink to `Model` implementations; surface sugar lowers to `Term`.
- Grades pervade the language — a cost paid once, not per construct.
- Bounded repetition and combinators are library surface, not kernel.

## Enforcement
- `engine/` — `Term`, the `Model` trait, the arrow-handler algebra.
- `crates/handl-kit-data-canonical/data/` — grades, sorts, eval schemes.
- Engine parity tests (`engine-test`) across interpreters.

## Design
+The graded-theory map, `Term` shape, syntax details, handler algebra and the
+frontier questions:
+[`design/0098-one-language-multiple-interpreters.md`](../design/0098-one-language-multiple-interpreters.md).
