# Why the arrow calculus kernel is closed

The arrow calculus kernel has exactly six combinators:

| Combinator    | Constructs                                       |
| ------------- | ------------------------------------------------ |
| `arr`         | Lift a pure function into an arrow               |
| `>>>` (`seq`) | Sequential composition                           |
| `first`       | Fan-out: run on the first of a pair              |
| `choice`      | Route on `Either`: `Left` to `l`, `Right` to `r` |
| `loop`        | Feedback (not iteration)                         |
| `app`         | Higher-order arrow application                   |

These are the only nodes the kernel knows about. The set is **closed**: new language features do not add new combinators. This is a deliberate design choice, not an oversight.

## Why closed

An open kernel — one where each new feature introduces a new combinator — accumulates special cases. Every combinator must be understood by every consumer of the kernel: the type checker, the refinement inference engine, every interpreter (tree-walk, bytecode, cranelift), the pretty-printer, the serialiser, and every handler that interprets a program. Each new combinator multiplies the surface area that those consumers must cover, and each one is a new place for the consumers to disagree.

A closed kernel inverts that pressure. New features are expressed as **combinations** of the existing six, or as **desugaring** to them, or as **normalisation** that eliminates the feature before the kernel ever sees it. The consumers stay simple because the kernel stays small. The cost of a new feature is paid once, at the boundary where it is translated into the kernel — not by every consumer downstream.

## The cost of an open kernel

Consider what would have happened if the module system (ADR-0069) had introduced two new kernel combinators — say `ModuleRef` and `Project` — to represent cross-file references and field projections:

- The `Scheme` enum gains two variants.
- Every `match` over `Scheme` in the codebase must handle them, or fail to compile.
- The refinement inference engine must learn what contracts `ModuleRef` and `Project` carry.
- Every interpreter must decide what `ModuleRef` _means_ at runtime — load a file? look up a registry? fail?
- Serialisation must round-trip them.
- The pretty-printer must render them.

None of those consumers should care about modules. Modules are a _source-level_ concern; by the time a program reaches the kernel, the question "which file did this arrow come from?" should already have been answered.

## How features enter without new combinators

The module system is the worked example. A file is an arrow; an import is a projection; an export is a public name. None of that is in the kernel. Instead:

- **Parsing** recognises `import` / `export` syntax and produces ordinary `Scheme` nodes, with module references and projections encoded as `arr` ops carrying string literals (`op: "module_ref"`, `op: "project"`).
- **Normalisation Strategy A** (`normalize_compile_time`) replaces those `module_ref` nodes with inlined records of the imported module's public arrows, and leaves `project` nodes as field accesses on the record.
- **Normalisation Strategy B** (`flatten_program`) inlines everything into one flat `Scheme` graph — no `module_ref`, no `project`, no external `app` references remain.

By the time the program reaches inference or interpretation, it is plain arrows. The kernel never learns that modules existed.

This pattern generalises. A new source-level feature enters the system at one of three boundaries:

1. **Desugaring** — the parser rewrites the feature into existing combinators. Cheapest; the kernel is unaware.
2. **Normalisation** — a pass over the `Scheme` graph eliminates the feature's encoding before downstream consumers run. Slightly more expensive; the encoding is visible between parsing and normalisation but not after.
3. **Handler interpretation** — the feature is expressed as an `arr` op whose meaning is supplied by a handler, not by the kernel. The kernel carries the op opaquely; a host that does not understand it refuses to interpret it.

None of these add a combinator. The kernel stays at six.

## When the set should grow

The set is closed, not frozen. A seventh combinator is justified only when a feature **cannot** be expressed as a combination of the existing six without losing a property the kernel must preserve — typically a property that every consumer relies on, such as "the graph is finite" or "feedback is well-founded". Such cases are rare and should be governed by an ADR. The default answer is no: find a combination, a desugaring, or a normalisation. The kernel earns a new combinator only when the alternatives have been exhausted and the cost of the open set is demonstrably outweighed.

## Relation to the handl language kernel

The handl _language_ kernel (the six field primitives described in [Why the language kernel is small](../explanations/small-kernel)) follows the same principle at a different layer: expressive power comes from composition, not from adding keywords. The arrow calculus kernel is the layer below — the calculus the language compiles _to_. Both layers are closed by design, and for the same reason: a small, fixed core keeps every consumer honest.

## Relation to ADR-0106

[ADR-0106: the object/arrow kernel](../../project/adrs/0106-object-arrow-kernel.md) grounds this calculus. It keeps the closed six-combinator set as the computational substrate and folds it into a single object world: arrows are objects, `sym()`/`sym(A)` provide nominal identity, and `f<...>` is the one dimension specialization form. The six combinators are unchanged — they are the arrow fragment of ADR-0106's conceptual kernel.
