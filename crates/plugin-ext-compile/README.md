# handl-plugin-ext-compile

Compile extension for the Handl platform (ADR-0114 Phase 14,
ADR-0128 §0.3).

This crate does **not** ship a capability plugin. It provides:

- Re-exports of the `handl-platform` compile operation —
  `Compile`, `CompileRequest`, `CompileBehavior`, `BasicCompiler`.
- `CompileFileRequest` (`src/request.rs`) — parses a
  `compile <target> <file> [options]` CLI invocation (`--output`,
  `--source-map`, `--debug`, `--format`, `--to`, `--emit`) and builds the
  `CompilerInput` (source, `Ast`, `Language`, `Term`, `TypecheckReport`)
  the compilers consume.

PL-004 T-101: the feature-gated `CompileDispatcher`/`linked_targets`
registry over the legacy `Compiler` trait objects is removed — the `hnl
compile` CLI (`ext-compile`) routes each target's kernel `Emitter` impl
through `hnl-tool-codegen`'s `Engine` directly. The six `tool-compile-*`
target crates still implement the `Compiler` trait in
`handl-plugin-compile` alongside their `Emitter` impls.
