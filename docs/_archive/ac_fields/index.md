# handl_fields

# handl-fields

Field extraction, interpolation, and validation orchestration
for self-describing documents.

This crate provides the domain logic for documents that declare
their own field schemas inline using a `$:` section. It
handles:

- Extracting the `$:` section from data.
- `${...}` interpolation supporting:
  - Namespace paths: `${doc.name}`, `${env.HOME}`, `${doc.author.email}`
  - Shorthand paths: `${name}` (equivalent to `${doc.name}`)
  - Function calls: `${now()}`, `${add(1, 2)}`, `${concat(doc.name, "!")}`
  - Nested calls: `${add(mul(2, 3), 4)}`
- Typed/validated function params via the `Schema<V>` system.
- User-defined function bodies that compose via
  host primitives — no Rust code needed for new functions.
  Functions are plain data (objects with a `body` field), callable
  via the `apply` host primitive.
- Scoped inheritance: `inheritable: true` fields propagate into nested
  objects; `mutable: false` fields cannot be overridden in child scopes.
- Detecting undeclared fields.

The crate is schema-agnostic — it does not validate fields itself.
Callers pass raw schema values and choose their own `Schema<V>`
implementation.

## Architectural rule

> **Field extraction and interpolation are domain logic, not CLI
> plumbing.** This crate keeps them testable and reusable.

## Modules

### [`handl_fields`](handl_fields.md)

*10 modules*

### [`computed`](computed.md)

*2 functions*

### [`expression`](expression.md)

*1 struct, 2 functions*

### [`extraction`](extraction.md)

*2 functions*

### [`interpolation`](interpolation.md)

*5 functions*

### [`merge`](merge.md)

*1 function*

### [`namespace`](namespace.md)

*2 functions*

### [`parser`](parser.md)

*3 functions*

### [`scope`](scope.md)

*2 functions*

### [`types`](types.md)

*1 enum, 1 type alias, 2 constants, 2 structs*

### [`validation`](validation.md)

*2 functions*

