+++
id = "ADR-0182"
title = "JSON Schema 2020-12 Reference & Composition Support in the Typechecker"
status = "open"
priority = "med"
+++

---
title: JSON Schema 2020-12 Reference & Composition Support in the Typechecker
type: adr
date: 2026-08-24
status: proposed
status_note: Proposed.
---


# JSON Schema 2020-12 Reference & Composition Support in the Typechecker

> Imported from `kit-data-prelude/project` — originally `ADR-0007` (global registry repo `lang`); renumbered `ADR-0182` in the host merge.

## Decision


### D1: Resolver strategy

Two options:

**Option A — Reuse `jsonschema` internals.**
If the `jsonschema` crate exposes a resolver, use it. This is ideal because
we keep one source of truth.

**Option B — Hand-rolled reference resolver.**
Implement RFC 6901 JSON Pointer parsing and JSON Schema 2020-12 anchor lookup
ourselves. No new dependency, but more code to maintain.

**Decision:** Phase 0 decides between them. If `jsonschema` is not usable,
proceed with a small hand-rolled resolver behind a clean trait so we can swap
it later.

### D2: `SchemaInterpreter` gets a `root`

The trait gains `interpret_with_root(schema, root)` with a default impl that
delegates to `interpret(schema)`. Callers that have a root document opt in;
everyone else keeps the old behavior.

```rust
pub trait SchemaInterpreter: Send + Sync {
    fn interpret(&self, schema: &JsonValue) -> Type;

    fn interpret_with_root(&self, schema: &JsonValue, _root: &JsonValue) -> Type {
        self.interpret(schema)
    }
}
```

### D3: JSON pointer parsing

If hand-rolling: strip the leading `#`, split by `/`, and unescape `~1` → `/`
and `~0` → `~` per RFC 6901. Anchor-only references (`#myAnchor`) look up the
anchor map. Local pointers (`#/$defs/foo`) walk the `root` `JsonValue`.

### D4: Root ownership

The caller is responsible for providing the correct root. The interpreter
resolves pointers only against the supplied `root` and does not fall back to a
larger document.

- Per-field `$defs`: pass the field's own schema as `root`.
- Document-level `$defs`: the caller builds a synthetic root object (e.g., a
  JSON object whose fields mirror the `$:` map) and passes that instead.

### D5: `$ref` alongside other keywords

JSON Schema 2020-12 allows `$ref` with siblings. The typechecker does not
support intersection types, so it flattens: resolve the `$ref`, collect the
remaining keywords, and if the combination narrows to a single concrete
`Type`, return it; otherwise return the resolved type and ignore conflicting
siblings. This is documented as a limitation.

### D6: Circular references

Track the current resolution path in a `HashSet<String>` of the raw `$ref`
strings currently being resolved. Push on entry, pop on exit. If a ref is
already in the active set, a cycle has been found and `Unknown` is returned.
This correctly handles DAGs where the same target is reached through multiple
non-cyclic paths.

Internal helper:

```rust
fn interpret_with_root_active(
    &self,
    schema: &JsonValue,
    root: &JsonValue,
    active: &mut HashSet<String>,
) -> Type;
```

The public `interpret_with_root` creates a fresh `HashSet` and delegates to
the helper.


## Why

The validation layer already supports JSON Schema Draft 2020-12 reference vocabulary (`$ref`, `$defs`, `$anchor`, `$dynamicRef`) and composition keywords (`allOf`, `anyOf`, `oneOf`, `not`, `if/then/else`) via the `jsonschema` crate at crates/dialect-canonical/src/schema_vocabulary/json_schema.rs:46. LANG.md documents these features as supported (§6.1).

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- Changes to runtime validation. The `jsonschema` crate still owns that.
- Adding first-class `Type::Intersection` or `Type::Union` variants. We flatten
- A complete, sound JSON Schema type system. We approximate and fail safe.
- Performance engineering for very large schema graphs beyond correctness.
- **No panics.** Malformed schemas, missing `$ref` targets, and cyclic

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (context, tables, phases, trade-offs): [`design/0182-schema-refs.md`](../design/0182-schema-refs.md).
