**handl_ast > metadata**

# Module: metadata

## Contents

**Traits**

- [`NodeMetadata`](#nodemetadata) - Metadata attached to an AST node.

---

## handl_ast::metadata::NodeMetadata

*Trait*

Metadata attached to an AST node.

`()` is the zero-sized "no metadata" instance. Concrete metadata types
(`Span`, `Attrs`, `Both`) implement this trait so they can be plugged
into `Node<K, V, M>` without changing the node shape.



