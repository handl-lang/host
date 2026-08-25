# handl_typecheck_import

# handl-typecheck-import

Cross-module type checking for the handl typechecker.

This crate follows `import()` expressions in merge fields, resolves
them via a [`ModuleResolver`], builds [`TypeEnv`]s from imported
schemas, and checks type compatibility across module boundaries.

It does **not** do any I/O directly — that's delegated to the
resolver implementation. This makes import type checking testable
without touching the filesystem.

## Architectural rule

> **Import type checking uses a resolver; the resolver does the I/O.**

The orchestrator (CLI, build tool) calls them in order:

```ignore
// 1. Build TypeEnv from local schemas
let env = build_env(&schemas, &interpreter);

// 2. Check local keys (expr, arg, merge, inherit, mutable)
// ... (via key crates)

// 3. Check imports (via resolver)
handl_typecheck_import::check_imports(
    &schemas, &merge_fields, &data,
    &resolver, &interpreter, &env, None,
);
```

## Modules

### [`handl_typecheck_import`](handl_typecheck_import.md)

*1 module, 2 functions*

### [`detect`](detect.md)

*1 function*

