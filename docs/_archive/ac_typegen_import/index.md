# handl_typegen_import

# handl-typegen-import

Import resolution for the handl typegen system.

This crate follows `import()` expressions in merge fields, resolves
them via a [`ModuleResolver`], and merges imported schemas into the
parent context.

It does **not** do any I/O directly — that's delegated to the
resolver implementation. This makes import resolution testable
without touching the filesystem.

Depends on `handl-typegen-core` (for filtering merge fields from
imported modules and reading `expr` values to detect `import()` calls).

## Architectural rule

> **Import resolution uses a resolver; the resolver does the I/O.**

The orchestrator (CLI, build tool) calls them in order:

```ignore
// 1. Core: build context from raw schemas
let mut ctx = TypegenContext::from_schemas(&extracted.schemas);

// 2. handl-key crates: filter + strip (pure)
handl_typegen_key_set_special::merge::filter_fields(&mut ctx, &extracted.merge);
ctx.strip_keys(&ac_keys);

// 3. Import resolution: follow imports, merge (via resolver)
handl_typegen_import::apply_imports(
    &mut ctx, &extracted, &data, &resolver, &vocab,
);

// 4. Backend: generate output (pure)
let schema = backend.generate(&ctx);
```

## Modules

### [`handl_typegen_import`](handl_typegen_import.md)

*1 function, 1 struct, 2 modules*

### [`import_detect`](import_detect.md)

*1 enum, 2 functions*

### [`import_resolve`](import_resolve.md)

*1 function*

