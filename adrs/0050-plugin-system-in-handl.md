---
title: Plugin System in Handl
type: adr
date: 2026-09-02
status: proposed
status_note: Proposed — the plugin system is a handl module over two Rust layers.
---

# ADR-0050: Plugin System in Handl

## Decision

1. **Two permanent Rust layers; everything above is handl.** The FFI/binding
   façade and the AST access below it stay Rust; the plugin system is handl.
2. **The plugin system is a handl module, not a Rust crate** — registry,
   contribute, dispatch and `host.load` fixpoint are handl code.
3. **`fmt` and `lint` are handl rule acceptors** with one rule file per AST
   kind.
4. **`handle fmt|lint` dispatches to the handl tooling kit**, not to Rust
   implementations.
5. **Build order**: binding façade → `ast.parse`/`ast.lex` access → handl
   `plugin` module → handl `fmt`/`lint` → per-kind rule files.

## Why

Plugins were Rust crates, so adding a tool or a rule kind meant a rebuild and
the plugin system could not be extended by users. A handl registry over a
small façade makes plugins data and keeps only the binding/AST layers in Rust.

## Not doing
- **Keep plugins as Rust crates** — the coupling this removes.
- **Put the registry in Rust** — it is pure handl over the façade.
- **Bolt fmt/lint onto Rust** — they are handl rule acceptors.

## Consequences
- Plugins are handl modules; adding a rule is adding a file.
- The façade is the only Rust growth point.
- The build order is staged and each step is usable.

## Enforcement
- `crates/plugin-ffi/` (façade), `crates/plugin-ext-kit/` (host.load).
- `handl-packages/plugin.hnl`, `fmt.hnl`, `lint.hnl` (handl modules).
- Rule files under the tooling kit.

## Design
+The two Rust layers, handl registry, rule acceptors and the build order:
+[`design/0050-plugin-system-in-handl.md`](../design/0050-plugin-system-in-handl.md).
