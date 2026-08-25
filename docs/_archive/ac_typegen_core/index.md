# handl_typegen_core

# handl-typegen-core

Schema-system-agnostic type generation core.

The core provides **generic primitives** and **plugin traits** for
building type generation contexts from field schema fragments. It has
zero knowledge of any handl-specific key (`expr`, `arg`, `merge`,
`inheritable`, `mutable`).

## Plugin traits

- [`KeyHandler`] — filters and strips special keys from the context.
  Implemented by `handl-typegen-key-set-special`.
- [`ImportResolver`] — resolves `import()` expressions and merges
  imported schemas. Implemented by `handl-typegen-import`.
- [`TypegenBackend`] — generates format-specific output.
  Implemented by `handl-typegen-backend-json-schema`.

The orchestrator (compiler pass, CLI) composes these plugins via
trait objects in `Config`.

## Architectural rule

> **The core knows nothing about handl keys, the output format, or the filesystem.**

## Dependency graph

```text
handl-typegen-core — generic primitives + plugin traits, no handl key knowledge
├── handl-typegen-key-set-special   — KeyHandler impl (knows all special handl keys)
├── handl-typegen-import            — ImportResolver impl (depends on handl-module-resolver-core)
└── handl-typegen-backend-json-schema — TypegenBackend impl (JSON Schema)

handl-module-resolver-core — ModuleResolver trait (not typegen-specific)
├── handl-module-resolver-file    — FileResolver (filesystem I/O)
├── handl-module-resolver-virtual — VirtualResolver (in-memory)
├── handl-module-resolver-http    — HttpResolver (HTTP via ureq)
└── handl-module-resolver-git     — GitResolver (Git via git2)
```

## Modules

### [`handl_typegen_core`](handl_typegen_core.md)

*1 module, 1 struct, 1 trait, 2 functions*

### [`TypegenContext`](TypegenContext.md)

*1 function*

### [`plugin`](plugin.md)

*1 function, 2 traits*

