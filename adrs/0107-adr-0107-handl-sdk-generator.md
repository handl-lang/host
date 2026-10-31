+++
id = "ADR-0107"
title = "ADR-0107: Handl SDK Generator"
status = "open"
priority = "medium"
+++

---
title: Handl SDK Generator
type: adr
date: 2026-09-13
status: accepted
status_note: Accepted — port-adapter SDK generation; every FFI arrow carries the `FFI` grade.
---

# ADR-0107: Handl SDK Generator

## Decision

1. **Port-adapter model**: a generated *port* is the abstract Handl interface;
   an *adapter* is the target-specific implementation.
2. **New crates**: `plugin-sdkgen` (shared IR), a backend per source format, an
   emitter per target, and `plugin-ext-sdkgen` for the CLI.
3. **Multi-backend by three dimensions** — source format, target ABI, emitter —
   each pluggable without touching the others.
4. **The C backend uses `libclang`**, not a hand-rolled parser.
5. **File output mirrors the module tree**; generated packages are ordinary
   repos with `pkg.hnl`/`src/`.
6. **Output form**: abstract `export` in the port, concrete
   `export … :=` in the adapter.
7. **All FFI arrows carry the `FFI` grade**, so effects are visible in the
   type.
8. **Mappings to Handl types/grades are data**, not backend code.
9. **Doc comments are emitted** from the source into the port.
10. **Type/grade composition and the CLI surface** are part of the generator:
    `sdkgen` subcommands per source/backend.

## Why

SDK generation had a hand-rolled parser and per-backend bespoke code, so every
new library or target meant new parsing and mapping logic. Port/adapter plus
data mappings makes a new target a backend, and a new source a parser backend.

## Not doing
- **A hand-rolled C parser** — `libclang` is the source of truth.
- **Per-backend bespoke mapping code** — mappings are data.
- **Ungraded FFI arrows** — every foreign call is `FFI`.

## Consequences
- `libclang` becomes a build dependency of the C backend.
- Generated ports/adapters are reviewable Handl, not opaque bindings.
- Doc comments survive generation; the CLI drives source/backend selection.

## Enforcement
- `sdkgen-c/`, `sdkgen-webidl/`, `sdkgen-file/`, `sdkgen-cli/`.
- `handl-packages/generated/` — ports and adapters.
- Generator tests per backend.

## Design
+The IR, three extensibility dimensions, output layout, mappings and CLI:
+[`design/0107-handl-sdk-generator.md`](../design/0107-handl-sdk-generator.md).
