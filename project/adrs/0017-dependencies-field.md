# Dependencies Field for Custom Fields

## Context

ADR-0016 introduced user-defined fields declared in `*.field.yml` files.
Those fields are loaded on demand by the field resolver, but nothing in a
document explicitly says _which_ custom fields it expects to use. This
makes it hard for tooling and the resolver to:

- Pre-load the right `*.field.yml` files before validation.
- Report a clear error when a document uses a custom field that is not
  installed or not in the search path.
- Understand which library modules rely on which custom fields for
  documentation, `handl info`, and the typegen.

A built-in `dependencies` field lets authors and libraries declare the
custom fields they need. The resolver can then load all of those fields up
front instead of discovering them one by one as it walks the document.

## Decision

Add a built-in `dependencies` field to handl documents. `dependencies` is
a list of custom field names that the document requires to be resolvable
before it is validated or evaluated.

### Where `dependencies` is allowed

- At the document top level.
- On a field declaration.
- In a `*.field.yml` file, when one custom field is built on top of another.
- In library modules, so the library can say it uses a custom field.

### Syntax

`dependencies` is a list of dotted field names. Names may be bare (`delay`)
for project-local fields or namespaced (`time.delay`, `resilience.timeout`)
for library fields.

```yaml
dependencies:
  - delay
  - resilience.timeout
  - fs.exists
```

### Resolution

1. The resolver collects `dependencies` from the document, then from each
   field declaration, then from nested modules.
2. It resolves each name with the field resolver from ADR-0016: built-in
   fields first, then `*.field.yml` files.
3. If a `dependencies` entry cannot be resolved, validation fails with a
   list of the missing fields and the files that requested them.
4. A `dependencies` entry that points to a built-in core field is accepted
   and ignored because the resolver already has it.
5. The list is de-duplicated across scopes; the same field is loaded once
   per document.

### Tooling integration

- `handl info <document>` shows the declared `dependencies`.
- `handl lint` (ADR-0015) may warn when a document uses a custom field but does
  not declare it in `dependencies`.
- `handl typegen` (ADR-0009) can produce a complete dependency graph.

## Consequences

- Documents and library modules become self-describing about the custom
  fields they need.
- The resolver can preload all required `*.field.yml` files in a single
  discovery pass instead of failing mid-resolution.
- Validation errors are clearer because the resolver can say which document
  requested a missing field.
- `handl info` and typegen can produce a complete dependency graph.
- Tooling can warn when a `dependencies` declaration is missing or stale.
- Library authors have another required convention to follow; `handl lint` can
  help enforce it.

## Status

Proposed
