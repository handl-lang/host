# handl_store

# handl-store

Content-addressable definition store for the handl language.

Definitions (functions, handlers) are identified by the hash of
their canonical JSON representation. Names are aliases that
resolve to hashes at compile time. The VM sees hash-encoded
strings — no AST change needed.

## Core types

- [`DefinitionHash`] — a content hash (SHA-256, hex-encoded).
- [`DefinitionStore`] — stores [`FunctionValue`]s by hash.
- [`NameRegistry`] — maps names to [`DefinitionHash`]es.

## Canonicalization

Hashing is deterministic: the same definition always produces the
same hash. Canonical JSON is produced by sorting object keys and
serializing with `serde_json` in canonical form.

## Modules

### [`handl_store`](handl_store.md)

*5 modules*

### [`canonical`](canonical.md)

*2 functions*

### [`eval_cache`](eval_cache.md)

*2 structs*

### [`eval_cache::EvalCache`](eval_cache/EvalCache.md)

*1 function*

### [`handler`](handler.md)

*2 type aliases, 5 functions*

### [`hash`](hash.md)

*1 struct*

### [`store`](store.md)

*2 structs*

### [`store::DefinitionStore`](store/DefinitionStore.md)

*1 function*

### [`store::NameRegistry`](store/NameRegistry.md)

*1 function*

