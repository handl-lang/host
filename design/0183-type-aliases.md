---
title: Generic Type Aliases for handl Type System — design detail
type: design
date: 2026-08-24
updated: 2026-10-29
refs: [ADR-0183]
---

> Imported from `kit-data-prelude/project/design/0008-type-aliases.md` — ADR-0008 renumbered to ADR-0183 in the host merge.

# Generic Type Aliases for handl Type System — design detail

Moved verbatim from ADR-0183 (ADR-0161 size budget). The ADR carries the rules;
this page carries the mechanism, tables and rationale.

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

## Why

Users cannot define named, reusable types in handl documents. A schema like `{ first: T, second: T }` must be repeated verbatim everywhere it's used. There is no way to give it a name (`Pair<T>`) and reference it from other field schemas. This makes complex schemas verbose and error-prone.

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- **No recursive aliases.** The depth limit prevents infinite loops, but
- **No alias scoping.** All aliases are document-level. No nested `$:` sections
- **No alias imports.** Aliases defined in an imported document are not
- **No runtime changes.** Aliases are erased at compile time — the runtime
- **No typegen emission of aliases.** The typegen pass skips `$defs` entries.

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (context, tables, phases, trade-offs): [`design/0183-type-aliases.md`](../design/0183-type-aliases.md).
