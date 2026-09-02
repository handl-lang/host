# Custom Fields via `*.field.yml`

## Context

handl documents declare fields with schemas, expressions, and modifiers.
The core language provides a fixed set of field-level keys: `type`, `expr`,
`arg`, `schema`, `inheritable`, `mutable`, `merge`. These are the language
primitives.

However, users often want higher-level convenience fields — `timeout`,
`delay`, `cache`, `notify` — that are sugar over the core primitives. Today,
each new convenience requires a change to the language kernel. This couples
high-level conveniences to the core, slows iteration, and makes the kernel
larger than it needs to be.

A field extension mechanism lets the community add new field-level keys
without touching the kernel. This is a language extensibility feature, not
a host-specific concern.

## Decision

Support user-defined fields declared in `*.field.yml` files. The core
language keeps a minimal set of built-in field keys; all higher-level or
convenience fields are loaded from these declarative files.

### Core built-in field keys

The core only implements the primitives needed to express any field:

- `type` — schema type declaration.
- `expr` — computed expression.
- `arg` — parameterized field declaration.
- `schema` — explicit schema override.
- `inheritable` — value flows to nested scopes.
- `mutable` — value cannot be overridden in nested scopes.
- `merge` — field's object value is spread into parent.

All other field keys are user-defined or library-defined.

### Field file format

```yaml
# fields/delay.field.yml
name: delay
type: sugar
schema:
  type: object
  required: [duration]
  properties:
    duration: { type: string }
expand: |
  seq:
    - sleep: duration
    - {{ step }}
```

### Field types

- `sugar` — the field expands to a core-language tree before evaluation. No
  runtime code is required. Best for combinators that can be expressed with
  core primitives.
- `handler` — the field is implemented by a host-provided handler registered
  at load time. The YAML file is the manifest; the handler is the
  implementation. Best for stateful or time-sensitive fields.

### Resolution

The field resolver resolves a field key in this order:

1. Built-in field keys.
2. `*.field.yml` files discovered in the document directory, then
   `libraries/fields/`, then configured field paths.
3. If no field is found, the document fails validation with a list of known
   fields.

### Location and naming

- Field files can live in a project directory, in `libraries/fields/<domain>/`,
  or in a configured field path.
- The file extension is `.field.yml` or `.field.yaml`.
- Field names are global within a document; use dotted names for library
  fields to avoid collisions, e.g. `time.delay` or `resilience.timeout`.

### Promotion path

A field that starts as `sugar` or `handler` can be promoted to a built-in
field key when it is:

- Used in a performance-critical path.
- Adopted widely across projects.
- Proven safe and stable over time.

This mirrors the promotion path for custom functions (ADR-0019): built-ins
emerge from the field library.

## Consequences

- The language kernel stays small. New conveniences do not require kernel
  changes.
- Users can create domain-specific fields without publishing a package or
  merging code.
- The same mechanism supports first-party library fields, so `timeout`,
  `delay`, `cache`, etc. can ship as `libraries/fields/` files.
- Tooling (linter, formatter, typegen) must discover and account for
  `*.field.yml` files.
- Stateful fields require a handler interface, which adds a small runtime
  API surface on the host.
- Field name collisions become possible; dotted namespacing and load-order
  rules are required.

## Status

Proposed
