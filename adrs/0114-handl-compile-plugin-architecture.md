# ADR-0114: Handl Compile Plugin Architecture

## Status

Proposed.

## Context

Handl currently has three execution engines (tree-walk, bytecode, cranelift)
that run Handl source at compile time and at runtime. It also has
`handl-emitters` for emitting `Language` data fragments and
`plugin-sdkgen-emitter-file` for writing generated SDK `.hnl` files. What it
does **not** have is a generic plugin architecture for compiling Handl source to
external target languages such as JavaScript, C, or WebAssembly.

## Decision

Introduce a `plugin-compile` family of crates.

- `plugin-compile` defines a `Compiler` trait and the target-agnostic
  `CompilerInput` (a parsed and type-checked Handl `Ast` plus optional metadata).
- Per-target compiler crates implement `Compiler`:
  - `plugin-compile-js` — emits JavaScript for browser / Deno / Node.
  - `plugin-compile-llvm` — emits native code and object files.
- The existing `handl-emitters` crate keeps its current role: emitting
  `Language` data fragments. It is **not** a source-to-source compiler.
- `host-cli` gains a `hnl compile <target> <file>` subcommand that dispatches
  to the appropriate compiler plugin.
- Compilers are selected by target name, not by file extension. The target
  triple / runtime is part of the compiler configuration.

## Consequences

- Compilers become first-class plugins alongside formatters, linters, and the
  test runner.
- The `Compiler` trait can be implemented without touching the Handl core, as
  long as the input AST and type information are exposed.
- The SDK generator (`plugin-sdkgen-*`) and the source-to-source compiler
  (`plugin-compile-*`) remain separate concerns: the former turns foreign APIs
  into Handl source; the latter turns Handl source into foreign code.
- A new dependency appears: the compiler needs access to the type-checked AST
  and a type/grade projection API from `plugin-engine-typecheck`.

## References

- [ADR-0107 — Handl SDK Generator](./0107-handl-sdk-generator.md)
- [ADR-0090 — Bootstrap CLI](./0090-bootstrap-cli.md)
