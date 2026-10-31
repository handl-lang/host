+++
id = "ADR-0028"
title = "ADR-0028: Pluggable Module Composition"
status = "open"
priority = "medium"
+++

---
title: Pluggable Module Composition
type: adr
date: 2026-08-24
status: proposed
status_note: Accepted — composition is per module; plugins declare their passes.
---

# ADR-0028: Pluggable Module Composition

## Decision

1. **Replace the static `frontend_for(format)` and monolithic `Dialect`** with
   per-module composition driven by a `module.toml` manifest.
2. **Every module folder may carry `module.toml`** (language kit + per-stage
   plugins); nested modules may carry their own, so a project can mix syntaxes
   and toolchains.
3. **Naming is `handl-*`** for kits, lexers, parsers, typecheckers, formatters,
   passes and multi-pass plugins — deterministic discovery, no collisions.
4. **A `Composer` assembles the pipeline** from the manifest plus each plugin's
   pass declarations, and validates coverage and conflicts before running.
   Stages invoke the `Composer`, not `frontend_for`.
5. **All passes are pluggable** — lexer, parser, resolve, typecheck, format,
   generate — each plugin declares the passes it implements; orchestrators run
   the configured plugins in order.
6. **AST strategy**: plugins may produce different ASTs; a `lower`/canonicalize
   pass to a shared IR is the recommended default, and opting out requires a
   complete AST-specific chain.

## Why

`frontend_for(format)` was a static map and `Dialect` was monolithic, so a new
syntax or a per-module toolchain meant editing the compiler. A manifest plus
pass declarations makes composition data and lets nested modules differ.

## Not doing
- **One global frontend** — modules may mix syntaxes.
- **A monolithic `Dialect` trait** — passes are plugins.
- **Free-form plugin names** — the `handl-*` scheme keeps discovery deterministic.
- **Heterogeneous ASTs downstream** — lower to a canonical IR or supply a full chain.

## Consequences
- Composition is data; the `Composer` validates before execution.
- Plugins declare passes once; manifests only override/extend.
- Lowering becomes the seam between plugin-specific and shared ASTs.

## Enforcement
- `crates/handl-frontend/` — the `Composer`.
- `handl-packages/*/module.toml` — per-module manifests.
- Plugin metadata (`passes = [...]`) and composer tests.

## Design
+Manifest format, naming, composer behavior, pass declarations and AST strategy:
+[`design/0028-pluggable-module-composition.md`](../design/0028-pluggable-module-composition.md).
