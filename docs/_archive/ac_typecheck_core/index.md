# handl_typecheck_core

# handl-typecheck-core

Typechecking primitives — schema-system-agnostic.

The core provides **generic types and traits** for static type
checking of handl documents. It has zero knowledge of any handl-specific
key (`expr`, `arg`, `merge`, `inheritable`, `mutable`).

handl key checks live in the `handl-typecheck-keys` crate, where each key
is a module (`arg`, `expr`, `merge`, `inherit`, `mutable`,
`termination`) that uses these primitives for its specific checks.
The orchestrator composes them.

## Architectural rule

> **The core knows nothing about handl keys, the schema language, or
> the filesystem.**

## Dependency graph

```text
handl-typecheck-core — generic types, traits, no handl key knowledge
├── handl-typecheck-infer           — expression type inference
├── handl-typecheck-keys            — handl key check modules
│   ├── arg                      — checks "arg" key
│   ├── expr                     — checks "expr" key
│   ├── inherit                  — checks "inheritable" key
│   ├── merge                    — checks "merge" key
│   ├── mutable                  — checks "mutable" key
│   └── termination              — checks "termination" key
├── handl-typecheck-import          — cross-module type checking (via resolver)
└── handl-typecheck-schema-json     — JSON Schema → Type mapping

handl-module-resolver-core — ModuleResolver trait (shared with typegen)
```

## Modules

### [`handl_typecheck_core`](handl_typecheck_core.md)

*7 modules*

### [`compat`](compat.md)

*2 functions*

### [`env`](env.md)

*1 struct*

### [`error`](error.md)

*1 enum, 1 type alias, 2 structs*

### [`interpreter`](interpreter.md)

*1 trait*

### [`trait_registry`](trait_registry.md)

*1 struct*

### [`trait_registry::TraitRegistry`](trait_registry/TraitRegistry.md)

*2 functions*

### [`type_`](type_.md)

*2 structs, 3 enums*

### [`unify`](unify.md)

*1 type alias, 5 functions*

