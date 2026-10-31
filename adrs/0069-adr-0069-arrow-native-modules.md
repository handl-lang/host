+++
id = "ADR-0069"
title = "ADR-0069: Arrow-Native Modules"
status = "open"
priority = "medium"
+++

---
title: Arrow-Native Modules
type: adr
date: 2026-09-01
status: accepted
status_note: Accepted — a file is an arrow; imports are projection.
---

# ADR-0069: Arrow-Native Modules

## Decision

1. **A file is an arrow.** A module is the arrow its file evaluates to, not a
   namespace or environment.
2. **Import normalizes to projection**: importing a name is selecting a field
   of the module arrow's result.
3. **Using a name is `App`** — a reference is application of a projection, not
   a scope lookup.
4. **Two caches: graphs and contracts.** Normalized module graphs and their
   contracts are cached separately.
5. **Normalization is one abstraction with many implementations**; the
   strategy is a choice, not a hardcoded path.
6. **No `export`, no visibility, no environments.** Those concepts disappear
   because a module is already a value.
7. **Multi-file programs are arrow composition** — linking is composition.

## Why

Modules were environments with export/visibility rules, and every consumer
re-implemented lookup. If a file is an arrow, import is projection and use is
application — two operations already in the calculus — so the module system
needs no separate semantics.

## Not doing
- **Keep export/visibility/environments** — they are the special cases arrows
  remove.
- **Eager evaluation of imports** — normalization is lazy and cached.
- **One hardcoded normalizer** — the strategy is an abstraction.

## Consequences
- Import/use lower to projection/application; no new kernel primitives.
- Two caches must agree; invalidation is per module graph/contract.
- Multi-file linking is composition; no linker phase.

## Enforcement
- `kernel/` — module loading and projection.
- `handl-runtime/` — the graph and contract caches.
- Module tests in `handl-runtime/tests/` and `engine-exec-*`.

## Design
+The seven rules in full, behavior examples and the five composition operations:
+[`design/0069-arrow-native-modules.md`](../design/0069-arrow-native-modules.md).
