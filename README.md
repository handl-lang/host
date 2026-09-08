# handl platform

`handl` is a small, embeddable, runtime-agnostic expression language and a pluggable language construction kit. This repository is the reference Rust platform: a Cargo workspace where the canonical syntax is assembled from tiny, swappable plugin crates (lexical rules, parser fragments, statements, formatting, diagnostics, LSP, runtime, and typecheck). The core contract lives in `handl-platform`, and everything from the canonical kit to the LSP host builds on top of it.

## Computational model

**Arrow calculus, not lambda calculus.** The kernel (ADR-0052) is six arrow-calculus primitives: `arr`/`>>>`/`first`/`choice`/`loop`/`app`. No lambda, no lambda application. `proc` desugars to arrow combinators; calling a `proc` is `app` (dynamic arrow invocation).

**Declarative, not imperative.** No mutation primitive — reassignment was deleted from the kernel. Binding (`x = y`) is immutable `Seq` + env extension. State goes through `loop` (feedback), not variable mutation.

**Descriptions, not execution.** `>>>` composes descriptions without running them. Handlers interpret the same description differently: runtime runs it, compiler lowers it, analyzer inspects it.

## High-level architecture

- `crates/platform` — core contracts: `Capability`/`CapabilityId`, `CapabilityRequest`/`CapabilityResponse`, `Plugin`, `PluginRegistry`, `Kit`, `KitRegistry`, `DefaultHost`, `PlatformError`.
- `crates/handl-language` — `Language`, `Ast`, `AstNode`, `AstRef`, and data-driven shape/type/eval/grammar fragments.
- `crates/handl-kit-*` / `kit-canonical` — canonical language data assembly and ready-made kit loading.
- `crates/handl-runtime` — runtime coordination, desugaring, and eval-engine dispatch.
- `crates/plugin-ext-*` — capability-contract surfaces and default rule-acceptor plugins: `diag`, `fmt`, `lint`, `kit`, `cli`, `lsp`, `eval`, `refinement`, `build`, `compile`, `plugin`.
- `crates/plugin-engine-*` — concrete tooling engines: `lex`, `parse`, `fmt`, `lint`, `typecheck`, `test`, `transform` (`transform-rewrite`, `transform-parallel`), `exec` (`tree-walk`, `bytecode`, `cranelift`).
- `crates/plugin-lib-*` — standard-library-style native/runtime modules (`stdlib`, `handl`, `scheduler`, `compress`, `kv`, `http`, `os`, `unicode`, `rand`, `time`, `tls`, `ws`, etc.).
- `crates/plugin-applier-*` / `plugin-load*` — composition resolution (`applier`, `applier-bootstrap`, `applier-default`) and plugin loaders (`native`, `wasm`, `web`).
- `crates/host-*` — end-user hosts: `host-cli` (`hnl`), `host-lsp` (LSP server), `host-tooling` (semantic tokens and LSP helpers).
- `crates/handl-value` / `handl-emitters` / `handl-test-utils` / `handl-defining-handl` — shared value model, emitters, conformance corpus, and language-in-handl experiments.
- `crates/crate-*` — vendored ICU/ECMA-402 utility crates.
- `handl-packages/` — pure-handl contracts and implementations (`.hnl` source). `data/` — YAML language fragments.

## Build and test

```bash
cargo check --workspace
cargo test --workspace
cargo build -p handl-host-cli   # produces ./target/debug/hnl
```

## The `hnl` CLI

`crates/host-cli` builds the `hnl` binary. Subcommands:

```text
hnl fmt        [--check] [--write] [paths...]
hnl lint       [--fix] [--fail-on-warn] [paths...]
hnl typecheck  [paths...]          (alias: hnl check)
hnl test       [--list] [--filter <pat>] [--tag <expr>] [--jobs N|auto]
               [--report human|json] [--changed] [--cache] [paths...]
hnl run        <file.hnl> [-- <args...>]
hnl eval       '<expression>'
hnl transform  <pass> <file.hnl> [--pipeline a,b,c] [--plan] [--trace]
               [--stats] [--emit tree|json|source] [--write]
hnl gen-grammar [output_path]
```

Exit codes: `0` success, `1` check/lint/test/fmt-check failure, `2` usage or parse error.

## Crate directory map

- `crates/handl-*` — language core: `handl-language`, `handl-kit-data`, `handl-kit-data-canonical`, `kit-canonical`, `handl-runtime`, `handl-value`, `handl-emitters`, `handl-test-utils`, `handl-defining-handl`.
- `crates/platform` — core platform contract (`Plugin`, `Capability`, `Host`).
- `crates/plugin-ext-*` — capability contracts and default category plugins (`diag`, `fmt`, `lint`, `kit`, `cli`, `lsp`, `eval`, `refinement`, `build`, `compile`, `plugin`).
- `crates/plugin-engine-*` — concrete language/tooling engines (`lex`, `lex-pattern`, `parse`, `parse-pratt`, `fmt`, `lint`, `typecheck`, `typecheck-nbe`, `test`, `transform`, `transform-rewrite`, `transform-parallel`, `exec`, `exec-tree-walk`, `exec-bytecode`, `exec-cranelift`).
- `crates/plugin-lib-*` / `crates/crate-*` — standard library, native ops, and utility libraries.
- `crates/plugin-applier-*` / `crates/plugin-load*` — applier composition and loaders.
- `crates/host-*` — CLI, LSP, and shared tooling host.
- `handl-packages/` — `.hnl` packages and contracts; `data/` — YAML language fragments.
- `docs/` — language and runtime design docs.
- `project/` — project notes and ADRs.

## Current status

- The pluggable expression pipeline and canonical kit are functional: lexing, parsing, error recovery, statement modules, formatting, and diagnostics all have working tests.
- `handl-kit-canonical` ships with optional typecheck and runtime plugin slots; the default `CanonicalKit` loads the canonical parser but does not enable full typecheck/runtime implementations unless they are configured.
- `handl-tooling` / `handl-host-lsp` provide LSP plumbing with decoupled completion, goto-definition, document-symbols, and incremental-sync plugin slots. Hover and real source-position resolution are still placeholders.
- Many compiler-pass, capability, and VM crates are partially implemented or still stubs; the workspace is in active construction.

See `.devin/AGENTS.md` for commands and `docs/LANG.md` / `docs/RUNTIME.md` for language and runtime details.
