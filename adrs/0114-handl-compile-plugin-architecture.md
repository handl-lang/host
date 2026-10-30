---
title: Handl Compile Plugin Architecture
type: adr
date: 2026-09-14
status: proposed
status_note: Accepted — compilers are per-target plugins over one `Compiler` trait.
---

# ADR-0114: Handl Compile Plugin Architecture

## Decision

1. **A `plugin-compile` crate defines `Compiler` and `CompilerInput`** (a parsed,
   type-checked `Ast` plus optional metadata); per-target crates implement it
   (`plugin-compile-js`, `plugin-compile-llvm`).
2. **`handl-emitters` keeps its role** — emitting `Language` data fragments; it
   is not a source-to-source compiler.
3. **`hnl compile <target> <file>` dispatches by target name**, not file
   extension; the target triple/runtime is part of the compiler configuration.
4. **The JS target exposes** `compile_to_string`, `emit_module` and
   `runtime_prelude`, with output modes `esm`, `commonjs` and `inline`.
5. **Value lowering** maps Handl primitives to JS, `Arr`/`Obj` to arrays/objects,
   `Bytes` to `Uint8Array`, `Handle` to an opaque reference, `Pair`/`Either` to
   tagged shapes, and arrows to closures.
6. **Foreign calls lower to `js.call(...)`** and inline to the native call when
   the object/method/descriptors are known at compile time;
   `--no-inline-foreign` keeps the runtime shim for dynamic calls.
7. **v1 emits no source maps**; `# sourceURL` may be added in `esm`/`inline`
   modes for debugging.

## Why

Compilation was target-specific code in the host, and emitters were mistaken
for a compiler. A `Compiler` trait with per-target plugins makes a target a
crate, keeps emitters at the data layer, and lets the CLI dispatch by name.

## Not doing
- **Compile in the host** — targets are plugins.
- **Use `handl-emitters` as the compiler** — it emits language data.
- **Select by file extension** — the target is explicit.

## Consequences
- A target is a crate implementing `Compiler`; the CLI stays thin.
- JS output has three modes and optional foreign inlining.
- No source maps in v1 — a debugging cost accepted.

## Enforcement
- `crates/plugin-compile/` — the trait and input.
- `compile-js/`, `compile-llvm/` — the targets.
- `host-cli` — the `compile` subcommand.

## Design
+The trait, JS API, output modes, lowering table and inlining rules:
+[`design/0114-handl-compile-plugin-architecture.md`](../design/0114-handl-compile-plugin-architecture.md).
