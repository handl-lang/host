+++
id = "ADR-0183"
title = "ADR-0183: Type Aliases in `$defs`"
status = "open"
priority = "med"
+++

---
title: Type Aliases in `$defs`
type: adr
date: 2026-08-24
status: proposed
status_note: Accepted — aliases are `$defs` + `$ref` + `args`.
---

# ADR-0183: Type Aliases in `$defs`

> Imported from `kit-data-prelude/project` — originally `ADR-0008` (global registry repo `lang`); renumbered `ADR-0183` in the host merge.

## Decision

1. **Type aliases live in `$defs` inside `$:`**, with an optional `typeParams`
   list — standard JSON Schema, no new top-level key or keyword.
2. **Aliases are type-only declarations**: they are not data fields, are not
   validated against data, and exist solely as `$ref` targets.
3. **References use standard `$ref` plus a handl `args` list**; `args`
   substitutes the alias's type parameters, and omitting it degrades them to
   `Unknown`.
4. **No new `Type` variant**: the resolved `$ref` produces a `Type` (possibly
   containing `Type::Var`), then `args` build the substitution.

## Why

Type aliases were special syntax, so every consumer had to know them. `$defs`
+ `$ref` reuses JSON Schema, and `args` adds exactly the generics the schema
layer lacks without inventing a second type system.

## Not doing
- **A new top-level alias key** — `$defs` already means this.
- **A new `Type` variant** — `$ref` + `args` resolves to existing types.
- **Validate `$defs` as data** — they are schema-only.

## Consequences
- Aliases compose with schema refs; no parallel resolver.
- Omitted args are permissive (`Unknown`), not an error.
- Tooling reads `$defs` like any schema.

## Enforcement
- `crates/handl-kit-data-prelude/` — schema parsing and `$ref` resolution.
- `data/` schema fixtures with aliases.
- Tests for alias substitution.

## Design
+The three decisions, examples and resolution rules:
+[`design/0183-type-aliases.md`](../design/0183-type-aliases.md).
