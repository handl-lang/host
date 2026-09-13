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

## Core forms

The kernel is described by a small set of forms; everything else derives from them (ADR-0106).

| Form       | Meaning                                                            |
| ---------- | ------------------------------------------------------------------ |
| `Obj`      | the world                                                          |
| `sym()`    | fresh nominal identity                                            |
| `sym(A)`   | fresh nominal identity with parent `A`                            |
| `A ~> B`   | arrow — the only computational primitive                          |
| `A ~[R]~> B` | graded arrow (`R` is the grade / grade list)                       |
| `f<...>`   | dimension specialization — lowers to application `f(X)` (ADR-0106 §11/§12); true specialization semantics remain |
| `a >>> b`  | composition                                                       |
| `f(x)`     | application                                                       |
| `(A, B)`   | product                                                           |

`Float := sym(Number)` makes `Float` a nominal child of `Number`, and the same parent graph decides runtime `instanceof`, static subtyping `<:`, and expected-type checks. Dimensions `f<...>` are the single specialization form for generics, dependent types, and refinements. The graded arrow is written `A ~[R]~> B` (legacy shorthand `A ~R~> B` is still accepted); the bracket holds the grade / grade list.

