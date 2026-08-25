# JSON Schema 2020-12 Reference & Composition Support in the Typechecker


## Context

The validation layer already supports JSON Schema Draft 2020-12 reference
vocabulary (`$ref`, `$defs`, `$anchor`, `$dynamicRef`) and composition
keywords (`allOf`, `anyOf`, `oneOf`, `not`, `if/then/else`) via the
`jsonschema` crate at
crates/dialect-canonical/src/schema_vocabulary/json_schema.rs:46.
LANG.md documents these features as supported (§6.1).

The typechecker does not. `interpret_schema` at
crates/typecheck-schema-json/src/interpret.rs:11-143 only matches on the
`type` keyword. Any schema with references or composition falls through to
`Type::Unknown`.

This means:
- A field with `{ "$ref": "#/$defs/positiveInt" }` validates correctly at
  runtime but its type is `Unknown` to the typechecker.
- `allOf`, `anyOf`, `oneOf` and conditional schemas silently degrade.
- Validation and typechecking can disagree, which is unexpected for users.
- There is no foundation for user-defined named types or recursive types.

### Goals

Use the full power of JSON Schema Draft 2020-12 in the typechecker by
resolving references and flattening composition into concrete `Type` values.
When a schema cannot be represented as a concrete `Type`, fall back to
`Unknown`. `Unknown` is compatible with everything, so this never rejects a
valid program and never introduces a breaking change.

### Design principles

1. **No breaking changes.** Add new trait methods with default impls; do not
   change public `Type` variants or APIs without a deprecation cycle.
2. **Fail safe.** Unresolvable, unsupported, or too-complex schemas return
   `Unknown`.
3. **Reuse, do not reimplement.** Where the `jsonschema` crate exposes
   resolution or composition utilities, use them. We keep one source of truth
   for JSON Schema semantics.
4. **Phased delivery.** The plan is split into independently shippable phases.
   If a phase is too much work, stop at the previous phase and treat the rest
   as follow-up.
5. **No surprises.** Every phase lists go/no-go criteria, tests, and explicit
   non-goals so there are no hidden expectations.

### In scope

- `$ref` resolution: local pointers, `$defs`, `$anchor`, and `$dynamicRef`.
- Sibling keywords alongside `$ref` (accumulate constraints, not ignore them).
- Cross-document / external `$ref` via a resolver hook.
- `allOf`, `anyOf`, `oneOf` where they can be flattened to a concrete `Type`.
- `not` and `if/then/else` where they produce a concrete `Type` or `Unknown`.
- Circular reference detection.
- Backward-compatible trait changes.

### Assumptions

- The `Type` enum has a finite, fixed set of concrete types and an `Unknown`
  fallback.
- The `jsonschema` crate remains the source of truth for runtime validation;
  the typechecker is a safe approximation.
- Callers are responsible for providing the correct `root` document.

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

## Consequences

- Changes to runtime validation. The `jsonschema` crate still owns that.
- Adding first-class `Type::Intersection` or `Type::Union` variants. We flatten
  to the most specific concrete `Type` and fall back to `Unknown`.
- A complete, sound JSON Schema type system. We approximate and fail safe.
- Performance engineering for very large schema graphs beyond correctness.

### Safety

The implementation must be safe in the type-system and runtime sense:

- **No panics.** Malformed schemas, missing `$ref` targets, and cyclic
  references return `Unknown`; they do not panic or abort.
- **No unbounded recursion.** The active-path set prevents cycles. Add an
  optional hard depth limit; if exceeded, return `Unknown`.
- **No implicit I/O.** The `SchemaInterpreter` never opens files, makes
  network requests, or reads environment variables. External `$ref`
  resolution is handled by an injected resolver controlled by the runner.
- **No breaking changes to existing code.** The `interpret_with_root`
  default impl and the `Unknown` fallback mean existing call sites keep
  working.
- **No false positives.** A schema that the validator accepts will never be
  rejected by the typechecker because of a missing type. `Unknown` is the
  worst-case result, and it is compatible with every type.

### Performance

- **O(pointer length)** resolution. JSON pointer walking is bounded by the
  pointer string length and the object depth. No full-document rescan on
  each `$ref`.
- **Anchor pre-scanning.** Build the anchor map once per `root` at the
  start of `interpret_with_root`, not on every anchor lookup.
- **Memoization.** Cache resolved `($ref`, `root`) pairs so a `$ref` hit
  several times in the same document (including DAGs) is resolved once.
  The cache is scoped to a single top-level call.
- **Short-circuited composition.** `allOf` stops at the first incompatible
  type; `anyOf`/`oneOf` returns a concrete type only if all alternatives
  agree; `not` returns `Unknown` immediately.
- **Bounded external resolution.** External `$ref` results are fetched once
  per URL per typecheck session; unresolved or unavailable external refs
  fall back to `Unknown`.
- **No pathological blow-up.** If the `$ref` chain depth exceeds a
  configurable limit, return `Unknown`.

### Risk assessment

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Cycle detection false-positives on DAGs | Medium | High | Use an active-path set, not a visited set; add DAG tests. |
| `Type` enum becomes too complex | Low | High | Do not add `Type::Intersection` or `Type::Union`; flatten and fall back to `Unknown`. |
| `jsonschema` resolver is not reusable | Low | Medium | Hand-roll a small resolver in Phase 0; keep it behind a trait. |
| External refs need module-loader changes | High | High | Gate Phase 3 on module system readiness. |
| Validator / typechecker mismatch for `$ref` + siblings | Medium | Medium | Document the simplification and fall back to `Unknown` for conflicts. |
| Callers forget to pass `root` | Medium | Medium | Default impl keeps old behavior; add integration tests that catch unresolved refs. |

### Success criteria

- The typechecker resolves all local `$ref` and `$defs` used in existing tests.
- No existing typecheck tests break.
- All new behavior is gated behind `interpret_with_root`; `interpret` continues
  to work exactly as before.
- Circular, external, or unsupported schemas produce `Unknown`, never errors or
  panics.
- `docs/LANG.md` accurately describes what the typechecker does and does not
  support.

## Implementation

### Phase 0 — Resolver discovery (optional, 1-2 days)

**Goal:** Decide whether the `jsonschema` crate can be reused for reference
resolution.

**Work:**
- Inspect the `jsonschema` crate API for an exposed resolver or compiled
  schema inspection.
- Spike a quick resolution test in a throwaway branch.
- Update D1 in this plan with the chosen strategy.

**Go/no-go:** We have a concrete resolver strategy.

**Output:** A one-paragraph addition to D1.

### Phase 1 — Trait extension + local `$ref` / `$defs` (shippable, low risk)

This is the core, corrected schema-refs work.

**Files:**
- `crates/typecheck-core/src/interpreter.rs` — add `interpret_with_root`.
- `crates/typecheck-schema-json/src/interpret.rs` — resolve local `$ref`.
- `crates/compiler-pass-typecheck/src/runner.rs` — wire root.
- `crates/compiler/src/fields.rs` — wire root.

**Implementation:**
- Add `interpret_with_root` with a default impl.
- Implement the local pointer resolver and active-path cycle detector.
- Wire `raw` as `root` in the runner and `fields.rs`.

**Tests:** `crates/typecheck-schema-json/src/lib_tests.rs`

- `{ "$ref": "#/$defs/pos", "$defs": { "pos": { "type": "integer" } } }` →
  `Type::Number`.
- `{ "$ref": "#/$defs/pair", "$defs": { "pair": { "type": "object",
  "properties": { "a": { "type": "string" } } } } }` → an object type with
  a string `a` property.
- Nested `$ref`: A → B → C, all resolve.
- DAG with shared target: A → C and B → C under the same root; both resolve
  `C` without degrading to `Unknown`.
- Circular `$ref`: A → B → A → returns `Unknown`.
- Missing `$ref` target → `Unknown`.
- `$ref` with a compatible sibling `type`.
- Regression: no-`$ref` schemas work unchanged via `interpret` and
  `interpret_with_root`.

**Go/no-go:** All new and existing tests pass, and `interpret` call sites are
unaffected by the default impl.

### Phase 2 — `$anchor` and `$dynamicRef` (shippable, moderate risk)

**Goal:** Resolve named and dynamic anchors.

**Files:** `crates/typecheck-schema-json/src/interpret.rs`.

**Implementation:**
- Pre-scan `root` for `$anchor` values and build an anchor map.
- Resolve `#myAnchor` through the map.
- For `$dynamicRef`, track the dynamic parent scope; unresolved dynamic refs
  return `Unknown`.

**Tests:**
- `#myAnchor` resolves to a type.
- `$dynamicRef: "#myAnchor"` with a known dynamic parent resolves; otherwise
  returns `Unknown`.

**Go/no-go:** If Phase 0 found a reusable `jsonschema` resolver, this is
small. If not, defer as a follow-up if it grows beyond the hand-rolled
resolver.

### Phase 3 — Cross-document / external `$ref` (gated, high risk)

**Goal:** Support references like `"./types.json#/$defs/foo"`.

**Implementation:**
- Pass a `RefResolver` trait or a resolver hook to `interpret_with_root`.
- The runner provides the resolver; default returns `None` → `Unknown`.

**Tests:**
- External ref resolves when a resolver is provided.
- Missing external ref → `Unknown`.

**Go/no-go:** Requires module-loader integration. Do not start until the
module system is ready.

### Phase 4 — Composition keywords (`allOf`, `anyOf`, `oneOf`, `not`)

**Goal:** Stop falling through to `Unknown` for common composition.

**Implementation:**
- `allOf`: merge subschemas. If the merged result has a single concrete
  `type`, return it; else `Unknown`.
- `anyOf` / `oneOf`: if all alternatives have the same `Type`, return it;
  else `Unknown`.
- `not`: return `Unknown` (always safe).
- Keep the `Type` enum unchanged; flatten to the most specific common `Type`.

**Tests:**
- `allOf` with `type: integer` and `minimum: 1` → `Type::Number`.
- `anyOf` with all alternatives as `string` → `Type::String`.
- `anyOf` with mixed types → `Unknown`.
- `not` → `Unknown`.

**Go/no-go:** No `Type` enum changes and all cases preserve `Unknown` fallback.

### Phase 5 — Conditional schemas (`if/then/else`)

**Goal:** Resolve conditionals when they produce a clear type.

**Implementation:**
- If the `if` condition cannot be evaluated at typecheck time, return
  `Unknown`.
- If `then` and `else` produce the same `Type`, return it; else `Unknown`.

**Tests:**
- `if/then/else` with both branches `string` → `Type::String`.
- Mixed branches → `Unknown`.

**Go/no-go:** Only if it does not require runtime value tracking.

### Phase 6 — Thread parent `root` into key crates

**Files:**
- `crates/typecheck-key-expr/src/check.rs`
- `crates/typecheck-key-arg/src/lib.rs`
- `crates/typecheck-key-inherit/src/check.rs`
- `crates/typecheck-key-inherit/src/walker.rs`
- `crates/typecheck-import/src/lib.rs`

**Implementation:**
- Where these crates receive a sub-schema extracted from a field, change
  `interpreter.interpret(schema)` to `interpreter.interpret_with_root(schema,
  parent_schema)`.
- Where the parent schema is not yet available, use `schema` as a temporary
  root to resolve per-fragment `$defs`. Thread the parent schema through as
  a follow-up.

**Go/no-go:** No existing tests break and `$ref` in args/expr/inherit/import
contexts begins to resolve.

### Phase 7 — Test mock interpreters

**Files:**
- `crates/typecheck-key-expr/src/lib_tests.rs`
- `crates/typecheck-key-arg/src/lib_tests.rs`
- `crates/typecheck-key-inherit/src/lib_tests.rs`

**Implementation:**
- Since `interpret_with_root` has a default impl, no immediate changes are
  required.
- Add `$ref` support to the mocks only when a test specifically needs it.

### Phase 8 — Documentation

**Files:**
- `docs/LANG.md` §9.3 (Schema interpreter):
  - Document that `$ref`, `$anchor`, and `$dynamicRef` are resolved.
  - Explain the `root` concept and per-field vs document-level `$defs`.
  - Document composition flattening and `Unknown` fallback.
  - Document that `$ref` with conflicting siblings is not fully intersected.
  - Note that circular refs degrade to `Unknown`.

- `0007-schema-refs.md` — this file, update status to "Implemented".

### Dependency graph

```
Phase 0 (resolver discovery)
  └── Phase 1 (local $ref / $defs)
        ├── Phase 2 ($anchor / $dynamicRef)
        │     └── Phase 3 (external $ref)
        ├── Phase 4 (composition)
        │     └── Phase 5 (conditionals)
        ├── Phase 6 (thread parent root)
        ├── Phase 7 (mocks)
        └── Phase 8 (docs)
```

Phases 1, 4, and 6 are the highest value for end-to-end typechecking. Phases
2, 3, and 5 add JSON Schema completeness and can be deferred if they become
too much work.

### Feature-completeness matrix

| JSON Schema 2020-12 keyword | Phase | Behavior |
|---|---|---|
| `$ref` | 1 | Resolve local `#/$defs/...` pointers. |
| `$defs` | 1 | Used as the local pointer target namespace. |
| `$anchor` | 2 | Resolve `#myAnchor` references. |
| `$dynamicRef` | 2 | Resolve within known dynamic scope, else `Unknown`. |
| `$id` | — | Document-only; not used for resolution in this plan. |
| `allOf` | 4 | Flatten to a single `Type` if possible, else `Unknown`. |
| `anyOf` | 4 | Return common `Type` if all alternatives agree, else `Unknown`. |
| `oneOf` | 4 | Same as `anyOf` for type inference; does not validate exclusivity. |
| `not` | 4 | Return `Unknown` (safe). |
| `if/then/else` | 5 | Return common `Type` of branches if determinable, else `Unknown`. |
| `type` | existing | Continue to match as before. |

## Status

Proposed.
