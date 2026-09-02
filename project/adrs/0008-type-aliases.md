# Generic Type Aliases for handl Type System


## Context

Users cannot define named, reusable types in handl documents. A schema like
`{ first: T, second: T }` must be repeated verbatim everywhere it's used. There
is no way to give it a name (`Pair<T>`) and reference it from other field
schemas. This makes complex schemas verbose and error-prone.

### Prerequisite

The [schema-refs ADR](0007-schema-refs.md) introduces `$ref`/`$defs` resolution in
the typechecker's `SchemaInterpreter`. That feature makes
`{ "$ref": "#/$defs/foo" }` produce a concrete `Type` instead of `Unknown`.

This plan builds on that foundation. `$ref` resolution gives us named schema
references. Adding `typeParams` to `$defs` entries gives us **generic** type
aliases — the `$ref` target is resolved, then type parameters are substituted.

### Scope

Type aliases are a **typechecker-only** feature. No runtime changes — the VM,
AST, host traits, value types, and continuation model are untouched. Like
generics, this is a static analysis layer addition.

## Decision

### D1: Type aliases live in `$defs` within `$:`

Type aliases are declared as `$defs` entries inside the `$:` section, with an
optional `typeParams` field. This uses standard JSON Schema `$defs` — no new
top-level key, no custom keyword.

```yaml
$:
  $defs:
    Pair:
      typeParams: [T]
      type: object
      properties:
        first: { type: var, name: T }
        second: { type: var, name: T }
  my_pair:
    $ref: "#/$defs/Pair"
    args: [{ type: string }]
```

`$defs` entries are **type-only declarations** — they are not data fields, not
validated against data, and not checked for undeclared fields. They exist
solely in the schema layer as `$ref` targets.

### D2: Referencing aliases — standard `$ref` + `args`

Aliases are referenced via standard JSON Schema `$ref`:

```json
{ "$ref": "#/$defs/Pair", "args": [{ "type": "string" }] }
```

- `$ref` — standard JSON Schema pointer to a `$defs` entry.
- `args` — handl extension. Type arguments to substitute for the alias's type
  parameters. If omitted, all type params degrade to `Unknown`.

The `$ref` resolution is already handled by the schema-refs feature. The
`args` field is the only addition — it's consumed after `$ref` resolution to
build a substitution.

No new `Type` variant is needed. The resolved `$ref` target produces a `Type`
(possibly containing `Type::Var` for type parameters). The `args` are
interpreted as `Type` values and used for substitution.

### D3: Alias table

A new `AliasTable` type maps `$defs` names to their declarations:

```rust
pub struct AliasEntry {
    pub type_params: Vec<TypeParam>,
    pub body: Type,
}

pub type AliasTable = BTreeMap<String, AliasEntry>;
```

The table is built during field extraction by scanning the `$defs` section of
the `$:` object. Each `$defs` entry with a `typeParams` field is a generic
alias. Entries without `typeParams` are non-generic aliases (named types with
no substitution step).

### D4: Eager expansion with `$ref` + substitution

Alias expansion happens in two stages:

**Stage 1 — `$ref` resolution (already implemented by schema-refs):**

The `SchemaInterpreter::interpret_with_root` resolves `$ref` pointers to
their `$defs` targets, producing a `Type` that may contain `Type::Var` for
unsubstituted type parameters.

**Stage 2 — alias expansion (this plan):**

After `$ref` resolution, if the resolved type contains `Type::Var` instances
that match alias type parameters, and the original schema had `args`:

1. Build the `AliasTable` from `$:` `$defs` entries with `typeParams`.
2. Extract `args` from the referencing schema (the one that had `$ref`).
3. Create a `Substitution` mapping each `type_param.name` to the
   corresponding arg `Type` (or `Unknown` if missing/fewer args).
4. Apply `substitute(&resolved_type, &subst)` to get the expanded type.
5. Recurse (the expanded type may itself contain `$ref` references to other
   aliases).

This reuses the existing `substitute` function from `typecheck-core/src/unify.rs`.

The key insight: `$ref` resolution and alias substitution are **decoupled**.
`$ref` gives you the body type. `args` + `substitute` gives you the
parameterized version. This means non-generic `$ref` targets work without any
alias machinery — they just resolve via `$ref` and produce a concrete type.

### D5: Non-generic aliases

A `$defs` entry without `typeParams` is just a named type — standard JSON
Schema `$ref` with no handl extension:

```yaml
$:
  $defs:
    Port:
      type: number
  server_port:
    $ref: "#/$defs/Port"
```

This works with `$ref` resolution alone — no `args`, no substitution, no
alias table entry needed. The `$ref` resolves to `{ type: number }` →
`Type::Number`.

### D6: Interaction with existing features

- **Schema-refs:** Alias expansion is layered on top of `$ref` resolution.
  `$ref` runs first (produces the body type), then substitution applies
  `args`. Non-generic `$defs` entries work with `$ref` alone.
- **Generics:** Alias expansion runs before generic function call checking.
  An alias can be used inside a function parameter type — the expansion
  produces a concrete type, then unification proceeds normally.
- **Merge fields:** Merge fields are excluded from `TypeEnv` construction
  (existing behavior). `$defs` entries are also excluded — they have no data.
- **Inheritance:** `$defs` entries are not inheritable. They are type-level
  constructs, not data fields.
- **Type generation:** The typegen pass should skip `$defs` entries (they
  have no data field to generate types for).
- **Validation:** The `jsonschema` crate already handles `$ref` to `$defs`
  for validation. `typeParams` and `args` are ignored by the validator —
  they're handl extensions. A generic alias used in validation will validate
  against the `$defs` body without substitution (treating `Type::Var` schemas
  as accepting anything). This is acceptable — type-level correctness is the
  typechecker's job, not the validator's.

## Consequences

- **No recursive aliases.** The depth limit prevents infinite loops, but
  recursive types (e.g. `type List<T> = { head: T, tail: List<T> }`) are not
  meaningfully supported — they expand to `Unknown` at the depth limit.
  Proper recursive types would require a different representation (e.g. named
  types in the `Type` enum) and are a separate feature.
- **No alias scoping.** All aliases are document-level. No nested `$:` sections
  with their own alias scope.
- **No alias imports.** Aliases defined in an imported document are not
  visible in the importing document. Each document has its own alias table.
- **No runtime changes.** Aliases are erased at compile time — the runtime
  never sees them.
- **No typegen emission of aliases.** The typegen pass skips `$defs` entries.
  Emitting TypeScript `type Pair<T> = ...` or Python `TypedDict` is a future
  enhancement.
- **No validation-level generic checking.** The `jsonschema` validator
  ignores `typeParams` and `args`. Generic alias correctness is the
  typechecker's responsibility.

### Risk assessment

- **Breaking changes:** None. `$defs` is standard JSON Schema — existing
  schemas without `$defs` are unaffected. The `args` field is an handl extension
  that's ignored when absent. The `AliasTable` field on `ExtractedFields` is
  a new field with a default-empty value.
- **Performance:** Alias expansion runs once per field during `TypeEnv`
  construction. For typical documents (10–50 fields, 0–5 aliases), this is
  negligible. The depth limit prevents pathological cases.
- **Backward compatibility:** Documents without `$defs` behave identically.
  The `AliasTable` is empty, `expand_aliases` is a no-op pass-through.
  Documents with non-generic `$defs` work via `$ref` resolution alone — no
  alias machinery needed.
- **Edge case — `$defs` entry without `typeParams` referenced with `args`:**
  The `args` are interpreted but have nothing to substitute against. The
  expansion produces `App(body, args)` which is left as-is (no matching alias
  entry). This is harmless — the typechecker treats it as an opaque type.
- **Edge case — `$ref` to `$defs` entry with `typeParams` but no `args`:**
  The `$ref` resolves to the body type containing `Type::Var` entries. With
  no `args`, there's no `App` wrapper, so `expand_aliases` doesn't touch it.
  The unresolved `Type::Var` entries behave as type variables — compatible
  with everything. This is graceful degradation.

## Implementation

### Phase 1 — `$defs` extraction (`fields`)

**Files:**

- `crates/fields/src/types.rs` — add `AliasEntry` struct and `AliasTable`
  type alias. Add `aliases: AliasTable` field to `ExtractedFields`.
- `crates/fields/src/extraction.rs` — after extracting field schemas, check
  if the `$:` object contains a `$defs` key. If so, iterate its entries and
  build the `AliasTable`:
  - Each `$defs` entry with `typeParams` → `AliasEntry { type_params, body }`
    where `body` is the entry interpreted as a `Type` (via
    `interpret_with_root`).
  - Each `$defs` entry without `typeParams` → not added to the table (it's
    a plain named type, handled by `$ref` resolution alone).
  - `$defs` itself is excluded from the `schemas` list (not a data field).

**Tests:** `crates/fields/src/extraction_tests.rs`

- Extract `$defs` with a generic alias `Pair<T>` — appears in `aliases`, not
  in `schemas`.
- Extract `$defs` with a non-generic `Port` — not in `aliases` (no
  `typeParams`), not in `schemas` (it's in `$defs`).
- `$defs` key itself does not appear in `schemas` used for data validation.
- No `$defs` → `aliases` is empty, no error.

### Phase 2 — `args` handling in `$ref` resolution (`typecheck-schema-json`)

**Files:**

- `crates/typecheck-schema-json/src/interpret.rs` — when resolving a `$ref`
  that has an `args` array alongside it:
  - Resolve the `$ref` target as usual (via `interpret_with_root`).
  - Interpret each entry in `args` as a `Type`.
  - Return `Type::App(Box::new(resolved_type), args)` — wrapping the resolved
    body in a `Type::App` so the expansion phase can find and substitute it.

  Alternatively, if the `$ref` target is in the `AliasTable`, the interpreter
  could apply substitution directly. But keeping it as `Type::App` decouples
  the interpreter from the alias table — the expansion phase handles it.

**Tests:** `crates/typecheck-schema-json/src/lib_tests.rs`

- `{ "$ref": "#/$defs/Pair", "args": [{ "type": "string" }] }` with root
  containing `$defs/Pair` → `App(Object({ first: Var("T"), second: Var("T") }),
  [String])`.
- `{ "$ref": "#/$defs/Port" }` (no `args`) → `Number` (plain `$ref`, no
  `App` wrapper).
- `{ "$ref": "#/$defs/Pair" }` (no `args`, but target has `typeParams`) →
  `Object({ first: Var("T"), second: Var("T") })` — unresolved vars, no
  `App` wrapper. Expansion phase will see unbound vars and degrade to
  `Unknown`.

### Phase 3 — Alias expansion (`typecheck-core`)

**New file:** `crates/typecheck-core/src/alias.rs`

```rust
/// Expand all alias applications in a type, using the alias table.
///
/// Recursively replaces `App(body, args)` with the substituted body,
/// where `body` contains `Var(name)` entries matching alias type params.
/// Unbound or missing args become `Unknown`.
///
/// Returns `Unknown` if expansion depth exceeds a limit (prevents infinite
/// recursion for self-referential aliases).
pub fn expand_aliases(ty: &Type, table: &AliasTable) -> Type
```

Implementation:

- Walk the `Type` tree.
- On `Type::App(body, args)`:
  - Expand `body` recursively first.
  - Expand each arg recursively.
  - If the expanded `body` contains `Type::Var` entries that match any
    `AliasEntry`'s type params in the table:
    - Build `Substitution` from `type_params` → args.
    - `substitute(&body, &subst)`.
    - Recurse on the result (may contain more alias applications).
    - Track depth; bail to `Unknown` after 32 levels.
  - If no matching alias entry, leave the `App` as-is (legitimate HKT
    application).
- On other `Type` variants, recurse into children.

**Tests:** `crates/typecheck-core/src/alias_tests.rs`

- Expand `App(Object({ first: Var("T"), second: Var("T") }), [String])` with
  table entry `Pair<T>` → `Object({ first: String, second: String })`.
- Expand with unbound arg (fewer args than params) → `Unknown` for missing
  args.
- Expand non-generic (no `App` wrapper, just a resolved type) → passthrough.
- Expand nested: `Array(App(Pair_body, [String]))` → `Array(Object({ first:
  String, second: String }))`.
- Depth limit: self-referential alias → `Unknown` after 32 levels.
- No alias table → `App` left as-is.

### Phase 4 — Integration into compiler/fields (`compiler`)

**Files:**

- `crates/compiler/src/fields.rs`:
  - After extracting fields, build the `AliasTable` from `extracted.aliases`.
  - When building `TypeEnv`, for each field schema:
    - Call `interpret_with_root(raw, raw)` (from schema-refs) to resolve
      `$ref` and produce a `Type` (possibly with `App` wrappers for `args`).
    - Run `expand_aliases` on the result to substitute type parameters.
  - Skip `$defs` entries when building the data validation schema list.
  - Skip `$defs` entries when collecting undeclared fields.

**Tests:** Integration tests in `crates/compiler/src/fields_tests.rs`:

- Document with `Pair<T>` in `$defs` and a field using `$ref` + `args` —
  field type is `Object({ first: String, second: String })`.
- `$defs` entry does not appear in `field_types` or trigger undeclared field
  errors.
- Non-generic `$defs` entry referenced via plain `$ref` — field type is the
  resolved type.
- Regression: document without `$defs` works unchanged.

### Phase 5 — Integration into typecheck runner (`compiler-pass-typecheck`)

**Files:**

- `crates/compiler-pass-typecheck/src/runner.rs`:
  - When building `TypeEnv` (line 61-64), call `interpret_with_root` and
    `expand_aliases` — same as Phase 4 but in the pipeline runner.
  - When building `FunctionTable` (line 112), pass root context for `$ref`
    resolution and run `expand_aliases` on param/return types.
  - Build `AliasTable` from `extracted.aliases` and thread it through.

**Tests:** Integration tests:

- Document with generic alias in `$defs` — typecheck resolves alias types
  correctly in expression checking.
- Expression referencing a field with `$ref` + `args` — inferred type matches
  the expanded alias.

### Phase 6 — Typegen integration (`typegen-core`)

**Files:**

- `crates/typegen-core/src/lib.rs` or `backend.rs` — skip `$defs` entries
  when generating types. `$defs` entries have no data — they should not
  produce emitted type definitions (or optionally, they could emit
  TypeScript type aliases / Python TypedDicts, but that's a future
  enhancement).

### Phase 7 — Documentation

**Files:**

- `docs/LANG.md`:
  - §6 (Field declarations): add subsection for `$defs` with `typeParams` as
    generic type aliases, with examples.
  - §9.3 (Schema interpreter): document the `args` extension to `$ref`.
  - §9 (Type system): add subsection on alias expansion.
- `0008-type-aliases.md` — this file, update status to "Implemented".

### Dependency graph

```
schema-refs ADR (prerequisite)
  └── Phase 1 ($defs extraction)
        └── Phase 2 (args in $ref resolution)
              └── Phase 3 (expand_aliases in typecheck-core)
                    └── Phase 4 (compiler/fields integration)
                          └── Phase 5 (typecheck runner integration)
                                ├── Phase 6 (typegen skip)
                                └── Phase 7 (docs)
```

Phases 1–3 are the core. Phases 4–5 make it usable end-to-end. Phases 6–7 are
polish.

## Status

Proposed.
