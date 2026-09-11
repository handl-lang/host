# Language

This page is the entry point for understanding the handl language.

handl is a JSON-native, functional language for transforming data. It is safe, sandboxed, and designed to run in many environments.

For a high-level introduction, read [What is handl?](./explanation/what-is-handl) and the [Language overview](./explanation/language-overview). To start writing, follow the [Getting started tutorial](./tutorials/getting-started).

## The object/arrow kernel

Underneath the surface, handl has a single semantic world: everything is an
object (`Obj : sym()`), and all computation is an arrow (`A ~> B`). There is no
separate type, function, effect, or proof universe — types, grades, proofs,
algebras, and capabilities are all ordinary objects, and generics, dependent
types, and refinements all ride on one dimension-specialization form, `f<...>`.
The VM primitives `sym()` and `sym(A)` create fresh nominal identity (with an
optional parent), and that one parent graph powers runtime `instanceof`, static
subtyping `<:`, and polymorphism. Programs are serializable arrow graphs
composed with `>>>` — no closures, no mutable bindings, no hidden captured
state.

For the full design, read [ADR-0106: The object/arrow kernel](../project/adrs/0106-object-arrow-kernel.md).

