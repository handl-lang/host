---
title: 'Task plan: ADR-0114 — Handl Compile Plugin Architecture'
type: task
date: 2026-09-14
status_note: Ready for implementation, one subagent per phase.
---

# Task plan: ADR-0114 — Handl Compile Plugin Architecture

Refs: `project/adrs/0114-handl-compile-plugin-architecture.md`

## Scope boundary

The new `plugin-compile-*` crates are **source-to-source / source-to-object**
compilers for the `hnl compile` CLI. They do **not** replace the existing
`plugin-lib-lang-compile` and `plugin-lib-lang-run-compiled` meta-host crates,
which remain in charge of the in-runtime `compile.compile` / `run_compiled`
operations.

## Crate module layout

### `crates/plugin-compile` (`handl-plugin-compile`)

```
src/
  lib.rs              // re-exports
  compiler.rs         // Compiler, CompileError
  config.rs           // CompilerConfig
  output.rs           // CompilerOutput, Artifact
  input.rs            // CompilerInput
  registry.rs         // CompilerRegistry
  value_abi.rs        // Value layout + helper_v2_* declarations
  source_map.rs       // SourceMap trait + V3 JSON
  debug_info.rs       // DebugInfo trait
  tests.rs
```

### `crates/plugin-compile-js` (`handl-plugin-compile-js`)

```
src/
  lib.rs
  compiler.rs         // JsCompiler
  config.rs           // JsConfig, JsTarget, JsOutputFormat
  output.rs           // JsOutput
  lower/mod.rs        // Term -> JsAst dispatcher
  lower/primitive.rs  // Arr, Const, Op, Seq
  lower/control.rs    // First, Choice, Loop
  lower/higher.rs     // App, Iter, Child, Handle
  lower/proof.rs      // Rewrite, Path, Equiv
  lower/op.rs         // Op lowering table
  emit/mod.rs         // JsAst -> string
  emit/module.rs      // file layout
  emit/source_map.rs  // source map generation
  runtime/mod.rs      // js client package prelude
  runtime/prelude.rs  // runtime helpers
  tests.rs
```

### `crates/plugin-compile-llvm` (`handl-plugin-compile-llvm`)

```
src/
  lib.rs
  compiler.rs         // LlvmCompiler
  config.rs           // LlvmConfig, LlvmTarget
  output.rs           // LlvmOutput
  target.rs           // target triple impls
  lower/mod.rs        // Term -> LLVM IR dispatcher
  lower/primitive.rs
  lower/control.rs
  lower/higher.rs
  lower/proof.rs
  lower/op.rs
  emit/mod.rs         // IR -> .o
  emit/object.rs      // ObjectWriter + inkwell finish
  emit/debug_info.rs  // DWARF line tables
  rt/abi.rs           // runtime helper imports
  tests.rs
```

### `crates/plugin-ext-compile` (`handl-plugin-ext-compile`) — rewired

```
src/
  lib.rs
  dispatcher.rs       // CompilerRegistry -> platform Compile
  request.rs          // CapabilityRequest -> CompilerInput
  tests.rs
```

## Core data model

- `CompilerInput` carries `Ast` + `Language` + root `Term` + `TypecheckReport`.
- `ValueAbi` is the shared runtime value layout (tag + payload) that both JS
  and LLVM lower to.
- `CompileError` covers `io`, `unsupported_target`, `lower`, `emit`, `link`,
  `invalid_input`.
- `CompilerRegistry` maps target name -> `Box<dyn Compiler>`.
- `CompilerConfig` and `CompilerOutput` are open traits; concrete types live in
  the per-target crates.

## Open targets / outputs

### JS

- `trait JsTarget` with `Deno`, `Bun`, `Node`, `Browser` impls.
- `trait JsOutputFormat` with `Esm`, `CommonJs`, `Iife`, `Inline` impls.
- `no_inline_foreign` flag for runtime `js` client fallback.

### LLVM

- `trait LlvmTarget` with concrete triple impls (`X86_64`, `AArch64`, etc.).
- `ObjectWriter` trait so tests can mock object emission.

## Implementation phases

### Phase 1 — Core contracts (`plugin-compile`)

- `Compiler`, `CompilerConfig`, `CompilerOutput`, `CompilerInput`, `CompileError`.
- `CompilerRegistry`.
- 100% unit tests for all public traits and registry paths.

### Phase 2 — Value ABI

- Define `ValueAbi` (tagged `serde_json::Value` layout, closure shape,
  pair/either representation).
- Declare `helper_v2_*` signatures.
- Tests for value layout and round-tripping.

### Phase 3 — JS primitive lowering

- `Arr`, `Const`, `Op`, `Seq` -> JS.
- `Deno` and `Node` targets; `Esm` and `CommonJs` formats.
- Source maps for the generated subset.
- Tests compare generated JS to expected strings.

### Phase 4 — JS control flow

- `First`, `Choice`, `Loop`, `Iter`, `App`.
- Tests for each combinator in isolation and in combination.

### Phase 5 — JS higher-order / proof terms

- `Child`, `Handle`, `Rewrite`, `Path`, `Equiv`.
- `Bun` and `Browser` targets, `Iife` and `Inline` formats.

### Phase 6 — JS source maps & debug

- Full V3 source maps, `sourceMappingURL`, span mapping.
- Error diagnostics that point back to Handl source.

### Phase 7 — JS security & fuzzing

- Foreign descriptor validation.
- `no_inline_foreign` guard.
- No `eval` / `new Function` in generated JS.
- Fuzz `Term` vs tree-walk / bytecode output.

### Phase 8 — LLVM primitive lowering

- `Arr`, `Const`, `Op`, `Seq` -> `inkwell` IR.
- `X86_64` and `AArch64` targets.
- Mock `ObjectWriter` for tests.

### Phase 9 — LLVM control flow & higher-order

- Same `Term` progression as JS.

### Phase 10 — LLVM object emission & debug info

- Real `.o` emission with `inkwell::object`.
- DWARF line tables via `inkwell::debug_info`.

### Phase 11 — LLVM performance

- `PassManager` with `instcombine`, `reassociate`, `gvn`.
- Benchmark vs tree-walk and bytecode.

### Phase 12 — Scalability

- `rayon` parallel per-module compilation.
- `plugin-exec-cache` incremental artifact cache.
- Streaming output for large modules.

### Phase 13 — Security & robustness

- Bounds checks on `Value` tags in LLVM.
- `#![forbid(unsafe_code)]` in `plugin-compile` and `plugin-compile-js`;
  unsafe isolated to `rt/` and `emit/` in `plugin-compile-llvm`.
- Graceful LLVM/inkwell build detection and helpful error messages.

### Phase 14 — Wiring & CLI

- `plugin-ext-compile` dispatcher.
- `host-cli` `hnl compile <target> <file> [--output] [--source-map] [--debug]`.
- End-to-end dogfood: compile a small `.hnl` to JS and Deno, and to `.o` with a
  C harness.

### Phase 15 — Acceptance

- `cargo test --workspace`, `cargo clippy`, `cargo fmt`.
- `cargo llvm-cov` or `cargo tarpaulin` per new crate.

## Verification after each subagent

```bash
cargo check -p handl-plugin-compile
cargo test -p handl-plugin-compile --lib
```

Run after every phase that touches a crate; `cargo check --workspace` after
Phases 1, 14, and 15.
