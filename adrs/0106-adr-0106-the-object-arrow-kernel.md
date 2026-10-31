+++
id = "ADR-0106"
title = "ADR-0106: The Object-Arrow Kernel"
status = "open"
priority = "medium"
+++

---
title: The Object-Arrow Kernel
type: adr
date: 2026-09-11
status: accepted
status_note: Accepted — one semantic world; arrows are the only primitive.
---

# ADR-0106: The Object-Arrow Kernel

## Decision

1. **One semantic world.** Values, objects, types and arrows share one value
   space; there is no separate type namespace.
2. **`sym()` / `sym(A)` is the nominal identity primitive** — names are values,
   and nominal typing is symbol identity.
3. **Arrows are the fundamental computational primitive.** There is no binding
   model and no mutation.
4. **Products and composition are the structural core**; `do` is sugar over
   composition.
5. **Dimensions `<>` specialize any value or arrow**, and generic objects use
   the same machinery.
6. **Graded arrows** carry effects in the grade, not in special forms.
7. **Algebra, capabilities and ordering** use the same objects and arrows — no
   interface/trait/impl layer.
8. **Proofs, refinements and dependent types are ordinary objects/arrows.**
9. **Polymorphism and Liskov are `sym`-based**; higher-order arrows and
   combinators need no new kernel primitive.
10. **Expected-type inference and runtime/static convergence** follow from the
    same arrow semantics.

## Why

The kernel had several semantic worlds — values, types, objects — plus a
binding model, so every new feature added a primitive. One world plus arrows
makes the surface derivable: products, dimensions, grades and proofs are
compositions, not kernel cases.

## Not doing

- **Separate type/object namespaces** — the split is what multiplied primitives.
- **A binding model or mutation** — both are derivable from arrows.
- **An interface/trait/impl layer** — `sym` and arrows express the same thing.
- **Add a primitive per feature** — features compose instead.

## Consequences
- The kernel stays small; inference does more of the work.
- Errors surface as type/arrow mismatches rather than special-case checks.
- Performance depends on lowering arrows well, not on kernel shortcuts.

## Enforcement
- `kernel/` — `Term`, `Value`, `sym` and the arrow constructors.
- `crates/handl-kit-data-canonical/data/` — the data that lowers to arrows.
- Conformance tests in the kernel and host crates.

## Design
+The full semantics, conformance notes and the serializable representation:
+[`design/0106-object-arrow-kernel.md`](../design/0106-object-arrow-kernel.md).
