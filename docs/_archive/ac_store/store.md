**handl_store > store**

# Module: store

## Contents

**Structs**

- [`DefinitionStore`](#definitionstore) - A content-addressable store of function definitions.
- [`NameRegistry`](#nameregistry) - Maps names to [`DefinitionHash`]es.

---

## handl_store::store::DefinitionStore

*Struct*

A content-addressable store of function definitions.

Stores [`FunctionValue`]s indexed by their [`DefinitionHash`].
The store is the backbone of content-addressing: definitions are
identified by hash, not by name. Names are resolved to hashes
separately via [`NameRegistry`].

# Determinism

The same `FunctionValue` always produces the same hash, so the
same definition inserted on two different machines will map to
the same entry. This enables cross-machine deduplication and
reproducible builds.

**Fields:**
- `defs: std::collections::HashMap<crate::hash::DefinitionHash, ac_runtime_values::FunctionValue>`

**Methods:**

- `fn new() -> Self` - Creates an empty store.
- `fn insert(self: & mut Self, function: FunctionValue) -> DefinitionHash` - Inserts a function definition and returns its hash.
- `fn get(self: &Self, hash: &DefinitionHash) -> Option<&FunctionValue>` - Looks up a definition by hash.
- `fn contains(self: &Self, hash: &DefinitionHash) -> bool` - Returns `true` if the store contains a definition with the
- `fn len(self: &Self) -> usize` - Returns the number of definitions in the store.
- `fn is_empty(self: &Self) -> bool` - Returns `true` if the store is empty.
- `fn iter(self: &Self) -> impl Trait` - Returns an iterator over all (hash, definition) pairs.
- `fn to_json(self: &Self) -> serde_json::Value` - Serializes the store to a JSON object mapping hash strings to
- `fn from_json(value: &serde_json::Value) -> Self` - Deserializes a store from a JSON object produced by

**Trait Implementations:**

- **Default**
  - `fn default() -> DefinitionStore`
- **Clone**
  - `fn clone(self: &Self) -> DefinitionStore`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_store::store::NameRegistry

*Struct*

Maps names to [`DefinitionHash`]es.

Names are aliases — the same definition can have multiple names.
At compile time, names in expressions are resolved to hashes,
so the VM sees hash-encoded strings.

**Fields:**
- `names: std::collections::HashMap<String, crate::hash::DefinitionHash>`

**Methods:**

- `fn new() -> Self` - Creates an empty registry.
- `fn register<impl Into<String>>(self: & mut Self, name: impl Trait, hash: DefinitionHash)` - Registers a name → hash mapping.
- `fn resolve(self: &Self, name: &str) -> Option<&DefinitionHash>` - Resolves a name to its hash.
- `fn contains(self: &Self, name: &str) -> bool` - Returns `true` if the registry contains the given name.
- `fn len(self: &Self) -> usize` - Returns the number of registered names.
- `fn is_empty(self: &Self) -> bool` - Returns `true` if the registry is empty.
- `fn iter(self: &Self) -> impl Trait` - Returns an iterator over all (name, hash) pairs.
- `fn to_json(self: &Self) -> serde_json::Value` - Serializes the registry to a JSON object mapping names to
- `fn from_json(value: &serde_json::Value) -> Self` - Deserializes a registry from a JSON object produced by

**Trait Implementations:**

- **Default**
  - `fn default() -> NameRegistry`
- **Clone**
  - `fn clone(self: &Self) -> NameRegistry`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



