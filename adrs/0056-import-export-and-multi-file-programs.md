---
title: Import / Export and Multi-File Programs
type: adr
date: 2026-09-02
status: proposed
status_note: Proposed — modules are evaluated environments.
---

# ADR-0056: Import / Export and Multi-File Programs

## Decision

1. **A module is an environment value** — evaluating a file produces the
   environment it defines.
2. **`import` resolves source and caches the resulting environment** (resolve →
   evaluate under the module `Language` → cache).
3. **`export` defines a module's public surface**; it is not a visibility
   keyword on individual bindings.
4. **`field-access` is the module access mechanism** — `module.name` reads the
   environment value.
5. **Import capability is a policy boundary**, not a language concept: the host
   decides what may be imported.
6. **The stdlib becomes `.hnl` files** evaluated into environments.
7. **The runtime exposes environments as values**, so modules are inspectable
   and composable.

## Why

`import`/`export` were `Foreign` host calls, so multi-file programs were a
special mechanism outside the language and the stdlib could not be `.hnl`.
Evaluated environments make a file an ordinary value and import a lookup.

## Not doing
- **Keep import/export as `Foreign`** — that is the special path this removes.
- **Per-binding visibility** — the export surface is the unit.
- **A separate stdlib loader** — the stdlib is `.hnl` like user code.

## Consequences
- Modules are values; import is resolve + evaluate + cache.
- The host controls import policy at the boundary.
- The stdlib shares the module mechanism.

## Enforcement
- `handl-runtime/` — environment values, import resolver.
- `kit-prelude-lang/` — the `.hnl` stdlib.
- Multi-file tests in `handl-runtime/tests/`.

## Design
+Environment values, import resolution, export surface and the loader:
+[`design/0056-import-export-and-multi-file-programs.md`](../design/0056-import-export-and-multi-file-programs.md).
