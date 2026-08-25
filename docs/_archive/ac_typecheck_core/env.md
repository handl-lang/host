**handl_typecheck_core > env**

# Module: env

## Contents

**Structs**

- [`TypeEnv`](#typeenv) - A type environment — maps field names to their declared types.

---

## handl_typecheck_core::env::TypeEnv

*Struct*

A type environment — maps field names to their declared types.

Supports scope chaining for inheritance: a child scope inherits
all bindings from its parent and can override individual fields.
Lookups walk the chain from child to parent.

**Fields:**
- `bindings: std::collections::HashMap<String, crate::Type>` - Field name → type in this scope.
- `parent: Option<Box<TypeEnv>>` - Parent scope, if any.

**Methods:**

- `fn new() -> Self` - Create an empty environment with no parent.
- `fn child(parent: TypeEnv) -> Self` - Create a child scope that inherits from `parent`.
- `fn insert<impl Into<String>>(self: & mut Self, name: impl Trait, ty: Type)` - Insert or update a binding in this scope.
- `fn extend<impl IntoIterator<Item = (String, Type)>>(self: & mut Self, entries: impl Trait)` - Insert multiple bindings from `(name, type)` pairs.
- `fn lookup(self: &Self, name: &str) -> Option<&Type>` - Look up a field's type, walking the scope chain.
- `fn contains(self: &Self, name: &str) -> bool` - Returns `true` if the field exists in this scope or any parent.
- `fn local_bindings(self: &Self) -> impl Trait` - Iterate over all bindings in this scope (not including parents).
- `fn all_bindings(self: &Self) -> HashMap<String, Type>` - Collect all bindings from this scope and all parents into a map.
- `fn all_bindings_iter(self: &Self) -> impl Trait` - Iterate over all bindings from this scope and all parents.

**Trait Implementations:**

- **Default**
  - `fn default() -> Self`



