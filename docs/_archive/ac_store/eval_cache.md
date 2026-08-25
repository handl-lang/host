**handl_store > eval_cache**

# Module: eval_cache

## Contents

**Structs**

- [`EvalCache`](#evalcache) - A memoization cache for evaluation results.
- [`EvalKey`](#evalkey) - A cache key for an evaluation result.

---

## handl_store::eval_cache::EvalCache

*Struct*

A memoization cache for evaluation results.

Maps `(DefinitionHash, args)` keys to result values. With
content-addressed definitions and pure functions, the cache is
sound: the same definition hash + same arguments always produces
the same result.

Operations (which signal and may have side effects) are never
cached. Only pure function calls and handler bodies are cached.

The cache is serializable as JSON, enabling persistence across
runs and sharing across machines.

**Fields:**
- `entries: std::collections::HashMap<EvalKey, ac_runtime_values::NonFunctionValue>`

**Methods:**

- `fn new() -> Self` - Creates an empty cache.
- `fn get(self: &Self, hash: &DefinitionHash, args: &[NonFunctionValue]) -> Option<&NonFunctionValue>` - Looks up a cached result by definition hash and arguments.
- `fn get_by_key(self: &Self, key: &EvalKey) -> Option<&NonFunctionValue>` - Looks up a cached result by a pre-computed [`EvalKey`].
- `fn insert(self: & mut Self, hash: &DefinitionHash, args: &[NonFunctionValue], result: NonFunctionValue)` - Records an evaluation result in the cache.
- `fn insert_with_key(self: & mut Self, key: EvalKey, result: NonFunctionValue)` - Records an evaluation result with a pre-computed key.
- `fn len(self: &Self) -> usize` - Returns the number of cached entries.
- `fn is_empty(self: &Self) -> bool` - Returns `true` if the cache is empty.
- `fn to_json(self: &Self) -> serde_json::Value` - Serializes the cache to a JSON value.
- `fn from_json(value: &serde_json::Value) -> Self` - Deserializes a cache from a JSON value produced by

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> EvalCache`
- **Clone**
  - `fn clone(self: &Self) -> EvalCache`



## handl_store::eval_cache::EvalKey

*Struct*

A cache key for an evaluation result.

Consists of a definition hash and the serialized argument values.
Two calls to the same definition with the same arguments produce
the same key, enabling memoization across machines.

**Fields:**
- `hash: crate::hash::DefinitionHash` - The content hash of the definition.
- `args_json: String` - Arguments serialized as a JSON array string.

**Methods:**

- `fn new(hash: &DefinitionHash, args: &[NonFunctionValue]) -> Self` - Creates a cache key from a definition hash and evaluated arguments.

**Traits:** Eq

**Trait Implementations:**

- **Hash**
  - `fn hash<__H>(self: &Self, state: & mut __H)`
- **PartialEq**
  - `fn eq(self: &Self, other: &EvalKey) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> EvalKey`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



