**handl_ast > node**

# Module: node

## Contents

**Structs**

- [`Node`](#node) - A generic AST node.

---

## handl_ast::node::Node

*Struct*

A generic AST node.

Parameterized by:
- `K` — the language-specific kind enum (expression, statement, type, ...).
- `V` — the value representation used by literals.
- `M` — the metadata attached to this node (default `()`).

The runtime can keep `M = ()` so the node is the same size as `K`.
Tools that need spans or attributes choose `M = Span`, `M = Attrs<A>`,
or a composed `Both<...>`.

**Generic Parameters:**
- K
- V
- M

**Fields:**
- `kind: K`
- `metadata: M`
- `_value: std::marker::PhantomData<V>`

**Methods:**

- `fn new(kind: K) -> Self` - Creates a node with no metadata.
- `fn with_metadata<N>(self: Self, metadata: N) -> Node<K, V, N>` - Replaces the node's metadata, changing the `M` type parameter.
- `fn map_kind<F, K2>(self: Self, f: F) -> Node<K2, V, M>` - Maps the kind while preserving metadata and the value type.

**Traits:** Eq

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &Node<K, V, M>) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> Node<K, V, M>`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> Node<K, V, M>`



