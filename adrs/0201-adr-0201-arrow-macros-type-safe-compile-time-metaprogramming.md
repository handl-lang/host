+++
id = "ADR-0201"
title = "ADR-0201: Arrow Macros — Type-Safe Compile-Time Metaprogramming"
status = "open"
priority = "med"
+++

---
title: Arrow Macros — Type-Safe Compile-Time Metaprogramming
type: adr
date: 2026-09-04
status: proposed
status_note: |-
  Proposed. Depends on [ADR-0052 — Arrow Calculus
  Kernel](./0052-arrow-calculus-kernel.md), [ADR-0044 — Language as
  Data](./0044-language-as-data-and-runtime-split.md), [ADR-0198 — Value
  Types as Data](./0198-value-types-as-data.md), [ADR-0054 — Generic
  Typecheck Engine](./0054-generic-typecheck-engine.md). Builds on
  [ADR-0081 — Data-Driven AST Child Roles](./0081-data-driven-ast-child-roles.md)
  and the `plugin-lib-lang-*` meta-function bridge (ADR-0083 §7).
---


# ADR-0201: Arrow Macros — Type-Safe Compile-Time Metaprogramming

> Imported from `kit-data-prelude/project` — originally `ADR-0084` (global registry repo `lang`); renumbered `ADR-0201` in the host merge.

## Decision

1. **Macros are arrows that transform arrows.** A macro is an arrow `Ast ~> Ast` (or more generally `Scheme A B ~> Scheme A' B'`). It is a first-class arrow value, typechecked like any other arrow. The compiler runs it at compile time, feeds it the AST of the macro call site, and splices the result.
2. **The `macro` declaration form.** A new top-level declaration, alongside `op`/`handl`/`cap`/`const`:
3. **Hygiene.** Macro hygiene prevents variable capture: a macro that introduces a variable `x` must not capture a caller's `x`. Handl's arrow calculus makes this natural:
4. **Type safety.** Macros are typechecked in two phases:
5. **Compile-time execution.** Macros run at compile time, not runtime. The execution model:
6. **Quotation and splicing.** Two syntax forms for building AST in macros:
7. **Pattern matching on AST.** Macros pattern match on AST structure. With ADR-0081 (child roles) and ADR-0200 (inductive types), this is ordinary pattern matching:
8. **What macros cannot do.** runtime with no `fs`/`net`/`libc` capabilities. A macro cannot read a file, make a network call, or fork a process. This prevents build-time side effects and keeps builds reproducible. fragment. It cannot modify the caller's bindings, inject imports, or change the language definition. The fragment is spliced in; the caller's scope is unchanged. rejects recursive macro expansions that don't terminate. A macro that expands to itself (or to a cycle of macros) is a compile error.
9. **Relationship to `const` (ADR-0083 §5.1).** `const` is the simplest macro — a nullary macro that produces a literal value:
10. **Relationship to `data` (ADR-0200).** `data` declarations (inductive types) are desugared to predicates, constructors, and recursors. Today this desugaring is hardcoded in the compiler. With macros, `data` can be a macro:

## Why

Handl is data-driven: a `Language` is YAML data, an `Ast` is a data structure, a `Scheme` (arrow graph) is data. The `plugin-lib-lang-*` crates expose `lex`/`parse`/`eval`/`compile`/`typecheck` as callable functions — handl code can already inspect and transform ASTs at runtime. But there is no **compile-time** mechanism for programs to generate code. Every abstraction must be expressed at runtime granularity.

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- **Type-safe metaprogramming.** Macros are arrows, typechecked before
- **No lambda opacity.** Arrow graphs are inspectable descriptions.
- **Homoiconic by construction.** `quote`/`splice` are syntactic sugar
- **Hygiene for free.** Arrow composition is structural (`>>>`), not
- **Compile-time sandboxed.** Macros run in a sandboxed runtime with no

## Enforcement

- `plugin-lib-lang-*` — where this is true.

## Design

The full record (mechanism, tables, proofs, trade-offs): [`design/0201-arrow-macros.md`](../design/0201-arrow-macros.md).
