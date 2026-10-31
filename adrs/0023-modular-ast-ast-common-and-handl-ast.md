+++
id = "ADR-0023"
title = "Modular AST: `ast-common` and `handl-ast`"
status = "open"
priority = "medium"
+++

---
title: 'Modular AST: `ast-common` and `handl-ast`'
type: adr
date: 2026-08-24
status: superseded
status_note: |-
  The trait-based `Node<K, V, M>` AST this ADR introduced (later renamed
  `handl-plugin-ast`) has been deleted. ADR-0044 replaces it with a
  serializable, string-keyed `Ast` arena (`AstNode { kind: String, span:
  Option<Span>, payload: Value }` + `AstRef` indices) in `handl-language`.
  The trait design could not satisfy ADR-0044's hard requirements:
  serde-serializable end to end, host-agnostic (crosses the wasm boundary),
  and data-driven (no `Box<dyn Behavior>`). See ADR-0044's "Supersession of
  the trait-based AST" note for the full rationale.
superseded_by: [ADR-0044]
---

# Modular AST: `ast-common` and `handl-ast`

## Context

The language model currently lives in `crates/ast` (`handl-ast`). It is intentionally dependency-free and minimal, holding only `Expr<V>` and `Field<V>` and a small set of constructors. This has kept the kernel stable, but it is starting to show three limitations:

1. **No shared node machinery.** Every `Expr` variant is hand-written and every constructor is hand-rolled. There is no common way to attach span, metadata, or custom fields without editing `Expr` itself.
2. **No statement layer.** The parser produces `Expr<NonFunctionValue>` directly and the typechecker operates on expressions. As the surface language grows, it needs statements (declarations, assignments, `handle` blocks, imports) and their own traversal utilities.
3. **No type AST in the same kernel.** Types live in `typecheck-core` (`Type`) and type generation lives in `typegen-core`. They are not composable with the expression model, so the parser/typechecker boundary duplicates concepts like generics and records.

The goal is to make the AST a set of **composable, configurable, and extensible** building blocks: a language-agnostic `ast-common` crate and a language-specific `handl-ast` crate that assembles expressions, statements, and types from those blocks.

## Decision

Introduce a new crate `crates/ast-common` (package `handl-ast-common`) and refactor `crates/ast` (`handl-ast`) to sit on top of it.

### `handl-ast-common` — the generic toolkit

`handl-ast-common` defines the structural primitives that every AST needs but no particular language owns. It has no dependencies and no language semantics.

- **`NodeMetadata`** — a marker trait (`Default + Clone + Debug`) for pluggable node metadata. `()` is the zero-sized "no metadata" instance. `handl-ast-common` ships small, composable metadata types: `Span`, `Attrs<A>`, and `Both<M1, M2>`, so consumers can mix and match without writing new structs.
- **`Node<K, V, M = ()>`** — a single AST node parameterized by its **kind** `K`, its **value type** `V`, and its **metadata** `M`.
  - `metadata` is not a fixed `Option<Span>`/attribute bag; it is whatever `M` the consumer chooses.
  - The runtime uses `Node<ExprKind, V, ()>` and pays zero bytes for metadata. The parser uses `Node<ExprKind, V, Span>`. The typechecker can use `Node<ExprKind, V, Both<Span, Attrs<TypeAnnotation>>>` if it wants more.
- **`NodeKind` traits** — `ExprKind`, `StmtKind`, `TypeKind` marker traits that language-specific enums implement.
- **Generic factory traits** — `ExprFactory`, `StmtFactory`, `TypeFactory`.
  - These let callers construct nodes through a trait interface rather than directly calling enum constructors. This is the "configurable (factory)" part: a dialect or embedded parser can provide a custom factory and still return `Node<CanonicalExprKind, V, M>`.
- **Traversal helpers** — `Walk`, `Fold`, and `Visit` traits that work for any `Node<K, V, M>` where `K` implements the kind trait.
  - Keeps `handl-ast` from accumulating ad-hoc visitor code.

```rust
// simplified sketch
pub trait NodeMetadata: Default + Clone + std::fmt::Debug {}

impl NodeMetadata for () {}

#[derive(Debug, Clone, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}
impl NodeMetadata for Span {}

#[derive(Debug, Clone, Default)]
pub struct Attrs<A: Clone + Default + std::fmt::Debug>(pub A);
impl<A: Clone + Default + std::fmt::Debug> NodeMetadata for Attrs<A> {}

#[derive(Debug, Clone, Default)]
pub struct Both<M1: NodeMetadata, M2: NodeMetadata> {
    pub first: M1,
    pub second: M2,
}
impl<M1: NodeMetadata, M2: NodeMetadata> NodeMetadata for Both<M1, M2> {}

pub struct Node<K, V, M: NodeMetadata = ()> {
    pub kind: K,
    pub metadata: M,
    pub _value: PhantomData<V>,
}

pub trait ExprFactory<E, V> {
    fn value(&self, value: V) -> E;
    fn call(&self, name: impl Into<String>, args: Vec<E>) -> E;
    fn array(&self, elements: Vec<E>) -> E;
    fn object(&self, entries: Vec<(String, E)>) -> E;
    fn handle(&self, body: E, clauses: Vec<HandlerClause<E>>) -> E;
}
```

### `handl-ast` — the handl language AST

`handl-ast` becomes the concrete, language-specific consumer of `handl-ast-common`. It re-exports the common machinery and defines the handl variants of expressions, statements, and types.

```
handl-ast
├── expr.rs    // Expr<V>, HandlerClause<V>
├── stmt.rs    // Stmt<V> (declaration, assignment, block, import, etc.)
├── ty.rs      // Type<V> (primitives, generics, records, arrays)
├── field.rs   // Field<V>
└── factory.rs // CanonicalExprFactory, CanonicalStmtFactory, CanonicalTypeFactory
```

- `Expr<V>` stays generic over the value representation, preserving the rule that the kernel is dependency-free.
- `Stmt<V>` is added as a top-level program element; statements can contain expressions but not the other way around.
- `Type<V>` moves here from `typecheck-core` and `typegen-core` so the parser, typechecker, and runtime all share the same type AST.
- The existing hand-rolled constructors on `Expr<V>` are kept as convenience methods but re-implemented through `CanonicalExprFactory` so there is one source of truth.

### Other crate impacts

- `handl-parser` continues to return `Expr<NonFunctionValue>` but can eventually expose a `Stmt`-based entry point.
- `handl-compiler-pass-parse` may return `Program<Stmt<NonFunctionValue>>` once statements exist.
- `handl-typecheck-core` and `handl-typegen-core` import `Type` from `handl-ast` instead of defining it locally.
- `handl-vm` and `handl-runtime` keep depending on `handl-ast` only; they never touch `handl-ast-common` directly.

## Consequences

### Positive

- **Composability.** New language features add a variant to a specific enum in `handl-ast`, not a new crate or a rewrite of the visitor.
- **Configurability.** Dialects and frontends can implement their own factories while still producing standard `Node` trees.
- **Extensibility.** `Node` metadata is fully pluggable. Tools compose `Span`, `Attrs`, or any `NodeMetadata` via `Both` and plug the result into `M` without changing `Node` or the factory traits.
- **Decoupling.** `handl-ast-common` has no dependencies and no language semantics. It could be reused for other intermediate representations or future languages.
- **One type AST.** Moving `Type` into `handl-ast` removes the current split between `typecheck-core` and `typegen-core`.

### Negative

- **Refactor churn.** `Type` currently lives in `typecheck-core` and `typegen-core`; consolidating it into `handl-ast` touches several crates and their tests.
- **Generic noise.** `Node<K, V, M>` adds a third generic parameter. Most code will alias it (`type ExprNode<V, M = ()> = Node<ExprKind, V, M>`), but the crate boundaries become noisier.
- **Runtime cost risk.** `Node<K, V, ()>` is the same size as `K` because `()` is a ZST. The runtime AST pays nothing for metadata unless it explicitly chooses `M = Span` or `Both<...>`. Cost is one generic parameter away from being opt-in.

### Resolved open questions

These are the long-term architectural answers; short-term implementation can still be staged inside `packages/handl/crates` and promoted later.

1. **Should `Stmt` own an `Expr` or share the same `Node` kind?**
   `Stmt` and `Expr` are separate syntactic categories. A `Stmt` is `Node<StmtKind, V, M>` and it **owns** `Node<ExprKind, V, M>` when it needs an expression. The generic `Node` container and metadata slot are shared, but the `Kind` enums are not. `Stmt::Expression(Node<ExprKind, V, M>)` makes any expression a statement, while `Stmt::Let { name: String, value: Node<ExprKind, V, M> }` is a statement that contains one.

2. **Should `handl-ast-common` live in the `handl` workspace, or at the Peregrin workspace level?**
   Long term, `ast-common` belongs at the **Peregrin workspace level** as a standalone, reusable crate because it is language-agnostic. The immediate step is to introduce it inside `packages/handl/crates/ast-common` so the existing Rust workspace can consume it without re-rooting the entire Cargo workspace. Once it has a consumer outside of `handl`, it should be promoted (e.g., `packages/ast-common` or `libraries/ast-common`).

3. **Is the `ast` crate intended to be renamed to `ast` without the `handl-` prefix?**
   No. Inside the `handl` Rust workspace, keep the existing `handl-*` convention: `handl-ast` for the handl language model and `handl-ast-common` for the generic toolkit. If `ast-common` is promoted to the Peregrin workspace, it drops the `handl-` prefix and becomes `ast-common`. The conceptual names in this ADR (`ast` and `ast-common`) refer to `handl-ast` and `handl-ast-common`.
