# handl platform

`handl` is a small, embeddable, runtime-agnostic expression language and a pluggable language construction kit. This repository is the reference Rust platform: a Cargo workspace where the canonical syntax is assembled from tiny, swappable plugin crates (lexical rules, parser fragments, statements, formatting, diagnostics, LSP, runtime, and typecheck). The core contract lives in `handl-platform`, and everything from the canonical kit to the LSP host builds on top of it.

## High-level architecture

- `handl-platform` — core contracts: `Capability`/`CapabilityId`, `CapabilityRequest`/`CapabilityResponse`, `Plugin`, `PluginRegistry`, `Kit`, `KitRegistry`, `Host`, `DefaultHost`, `LanguageHost`, `Boundary`, `PlatformError`.
- `handl-expr` — pluggable expression-to-AST pipeline: `ExpressionPipeline`, `ModulePipeline`, `TokenRule`, `ParseContext`, and the statement/expression fragment traits. `Stmt` wraps a `StmtKind`; `Expr<T>` is in `handl-ast-canonical`; `Expr<NonFunctionValue>` (from `handl-vm-values-json`) is the canonical expression type.
- `handl-plugin-lexer` / `handl-lexer-canonical` — token and token-kind abstractions and the canonical token set.
- `handl-plugin-ast` / `handl-ast-canonical` — shared AST shapes and the canonical expression tree.
- `crates/plugin-*` — individual syntax and tooling plugins: literals, identifiers, delimiters, operators, primary/postfix/infix fragments, statements, comments, formatting, diagnostics, and LSP features.
- `handl-plugin-runtime` / `handl-typecheck-*` — runtime and typecheck extension points and concrete implementations.
- `handl-tooling` — `ToolingHost`: decouples parsing (`CanonicalKit`), formatting, diagnostics, and LSP services (completion, goto-definition, document symbols, incremental sync).
- `handl-host-lsp` — LSP adapter that dispatches to `ToolingHost` and converts results to `lsp-types`.
- `handl-vm*` / `handl-compiler*` / `handl-frontend*` — virtual machine, multi-pass compiler, and data frontends.
- `crates/ext-*` — capability metadata and command builders used by the CLI and other hosts.
- `crates/loader*`, `module-resolver*`, `lib-*` — plugin loaders, module resolvers, and standard-library-style runtime libs.

## Build and test

```bash
cargo build
cargo test -p handl-kit-canonical
cargo test -p handl-platform
cargo test -p handl-host-lsp
cargo test -p handl-expr
```

The workspace can also be checked with `cargo check --workspace` and tested with `cargo test --workspace` when you have time.

## Crate directory map

- `crates/ast*` — AST types.
- `crates/ext-*` — capability contracts and default plugin categories.
- `crates/host-cli` — `hnl` binary.
- `crates/compiler*` — compiler pipeline and passes.
- `crates/dialect*` — dialect traits and canonical dialect spec.
- `crates/expr` — pluggable expression pipeline.
- `crates/frontend*` — JSON/YAML/Toml frontends.
- `crates/host-lsp` — LSP server host.
- `crates/kit-canonical` — canonical ready-made kit.
- `crates/lexer*` — lexer framework and canonical token set.
- `crates/lib-*` / `crates/crate-*` — runtime/utility libraries.
- `crates/loader*` — plugin loaders (native, wasm, web).
- `crates/module-resolver*` — module resolution strategies.
- `crates/parser*` — parser framework and canonical parser.
- `crates/platform` — core platform contract.
- `crates/plugin-*` — language and tooling plugins.
- `crates/plugin-runtime` — runtime trait.
- `crates/tooling` — `ToolingHost`.
- `crates/typecheck*` / `crates/typegen*` — typecheck and schema generation.
- `crates/vm*` — virtual machine and backends.
- `docs/` — language and runtime design docs.
- `project/` — project notes and ADRs.

## Current status

- The pluggable expression pipeline and canonical kit are functional: lexing, parsing, error recovery, statement modules, formatting, and diagnostics all have working tests.
- `handl-kit-canonical` ships with optional typecheck and runtime plugin slots; the default `CanonicalKit` loads the canonical parser but does not enable full typecheck/runtime implementations unless they are configured.
- `handl-tooling` / `handl-host-lsp` provide LSP plumbing with decoupled completion, goto-definition, document-symbols, and incremental-sync plugin slots. Hover and real source-position resolution are still placeholders.
- Many compiler-pass, capability, and VM crates are partially implemented or still stubs; the workspace is in active construction.

See `.devin/AGENTS.md` for commands and `docs/LANG.md` / `docs/RUNTIME.md` for language and runtime details.
