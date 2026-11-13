# handl-plugin-ext-compile

Compile extension for the Handl platform (ADR-0114 Phase 14,
ADR-0128 §0.3).

This crate does **not** ship a capability plugin. It provides:

- Re-exports of the `handl-platform` compile operation —
  `Compile`, `CompileRequest`, `CompileBehavior`, `BasicCompiler`.
- `CompileDispatcher` (`src/dispatcher.rs`) — dispatches
  `compile(target, input, config)` over a `CompilerRegistry`. Compiler
  targets are not a static table here: each lives in its target repo
  (`compile-js`, `compile-llvm`, `compile-handl`, `compile-term`,
  `compile-dot`, `compile-sexp`) and is opted in by a Cargo feature on the
  host's manifest (`linked_targets()` registers one per enabled feature).
  `with_registry` is the injection seam for hosts that assemble their own
  target set.
- `CompileFileRequest` (`src/request.rs`) — parses a
  `compile <target> <file> [options]` CLI invocation (`--output`,
  `--source-map`, `--debug`, `--format`, `--to`, `--emit`) and builds the
  `CompilerInput` (source, `Ast`, `Language`, `Term`, `TypecheckReport`)
  the compilers consume.
