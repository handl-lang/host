# handl platform

`handl` is a small, embeddable, runtime-agnostic expression language and a pluggable language construction kit. This repository is the reference Rust platform: a Cargo workspace where the canonical syntax is assembled from tiny, swappable plugin crates (lexical rules, parser fragments, statements, formatting, diagnostics, LSP, runtime, and typecheck). The core contract lives in `handl-platform`, and everything from the canonical kit to the LSP host builds on top of it.

## Computational model

**Arrow calculus, not lambda calculus.** The kernel (ADR-0052), grounded by [ADR-0106](project/adrs/0106-object-arrow-kernel.md), is six arrow-calculus primitives: `arr`/`>>>`/`first`/`choice`/`loop`/`app`. No lambda, no lambda application. `proc` desugars to arrow combinators; calling a `proc` is `app` (dynamic arrow invocation).

**One object world.** Everything is an object (`Obj : sym()`); the arrow `A ~> B` is the only computational primitive. `sym()` / `sym(A)` mint nominal identity, and the same `sym` parent graph powers `instanceof`, `<:`, and polymorphism — there is no separate type, function, effect, or proof universe.

**Declarative, not imperative.** No mutation primitive — reassignment was deleted from the kernel. Binding (`x = y`) is immutable `Seq` with no lexical environment to extend (see ADR-0106). State goes through `loop` (feedback) or explicit threading (`State ~> (State, Result)`), not variable mutation.

**Descriptions, not execution.** `>>>` composes descriptions without running them. Handlers interpret the same description differently: runtime runs it, compiler lowers it, analyzer inspects it.

## High-level architecture

- `crates/platform` — core contracts: `Capability`/`CapabilityId`, `CapabilityRequest`/`CapabilityResponse`, `Plugin`, `PluginRegistry`, `Kit`, `KitRegistry`, `DefaultHost`, `PlatformError`.
- `crates/handl-language` — `Language`, `Ast`, `AstNode`, `AstRef`, and data-driven shape/type/eval/grammar fragments.
- `crates/handl-kit-*` / `kit-canonical` — canonical language data assembly and ready-made kit loading.
- `crates/handl-runtime` — runtime coordination, desugaring, and eval-engine dispatch.
- `crates/plugin-ext-*` — capability-contract surfaces and default rule-acceptor plugins: `diag`, `fmt`, `lint`, `kit`, `eval`, `refinement`, `compile`, `plugin` (`cli`/`lsp` moved to their host repos in ADR-0128 Phase 8).
- `crates/plugin-engine-*` — concrete tooling engines: `lex`, `parse`, `fmt`, `lint`, `typecheck`, `test`, `transform` (`transform-rewrite`), `exec` (`tree-walk`, `bytecode`, `cranelift`).
- `crates/plugin-lib-*` — standard-library-style native/runtime modules (`stdlib`, `handl`, `scheduler`, `compress`, `kv`, `http`, `os`, `unicode`, `rand`, `time`, `tls`, `ws`, etc.).
- `crates/plugin-applier-*` / `plugin-load*` — composition resolution (`applier`, `applier-bootstrap`, `applier-default`) and plugin loaders (`native`, `wasm`, `web`).
- `crates/host-*` — `host-tooling` (semantic tokens and LSP helpers); the end-user hosts moved out in ADR-0128 Phase 8 — `host-cli` (`hnl`) lives in the sibling **`cli`** repo, `host-lsp` (LSP server) in **`lsp`**.
- `crates/handl-value` / `handl-emitters` / `handl-test-utils` / `handl-defining-handl` — shared value model, emitters, conformance corpus, and language-in-handl experiments.
- `crates/crate-*` — vendored ICU/ECMA-402 utility crates.
- `handl-packages/` — pure-handl contracts and implementations (`.hnl` source). `data/` — YAML language fragments.

## Build and test

```bash
cargo check --workspace
cargo test --workspace
# `hnl` moved to the sibling `cli` repo (ADR-0128 Phase 8):
(cd ../cli && cargo build -p handl-host-cli)   # produces ../cli/target/debug/hnl
```

## The `hnl` CLI

The `hnl` binary lives in the sibling **`cli`** repo (`crates/host-cli`).
Subcommands:

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

- `crates/handl-*` — language core: `handl-language`, `handl-kit-data`, `handl-kit-data-canonical`, `kit-canonical`, `handl-runtime`, `handl-value`, `handl-emitters`, `handl-test-utils`, `handl-defining-handl`, `handl-kit-arrow-lower`.
- `crates/platform` — core platform contract (`Plugin`, `Capability`, `Host`, `Kit`); `crates/platform-tests` — its integration tests.
- `crates/plugin-engine` — the generic engine substrate (staged `Behavior` machinery).
- `crates/plugin-engine-*` — concrete engines, all data-driven off `Language`:
  - front end — `lex`, `lex-pattern`, `parse`, `parse-pratt`
  - tooling — `fmt`, `lint`, `typecheck` (bootstrap checker), `typecheck-handl` (the `.hnl` model's inference engine), `test`
  - transforms — `transform`, `transform-rewrite`
  - execution — `exec`, `exec-tree-walk`, `exec-bytecode`, `exec-cranelift`
- `crates/plugin-ext-*` — capability contracts and the default provider per category (`diag`, `fmt`, `lint`, `typecheck`, `test`, `cli`, `lsp`, `eval`, `refinement`, `compile`, `kit`, `plugin`, `sdkgen`). The tooling four (`fmt`/`lint`/`typecheck`/`test`) are thin `Plugin` adapters over their `plugin-engine-*` engine — see ADR-0085.
- `crates/plugin-exec-cache` / `plugin-compile-*` — the exec result cache and the compiler back ends (`dot`, `js`, `llvm`, `sexp`, `term`, `handl`).
- `crates/plugin-sdkgen*` — SDK generators (and their back ends/emitters).
- `crates/plugin-lib-*` / `crate-*` — standard library, native ops, and utility libraries.
- `crates/plugin-applier-*` / `plugin-load*` / `plugin-loader-native` — applier composition and the native loader; `plugin-module` — module plumbing.
- `crates/host-*` — `host-tooling` (the shared tooling/LSP host contract). `host-cli` + `plugin-ext-cli` moved to the **`cli`** repo; `host-lsp` + `plugin-lsp` + `plugin-lsp-std` + `plugin-ext-lsp` moved to the **`lsp`** repo (ADR-0128 Phase 8).
- `crates/bench` / `ext-tests` — criterion benchmarks and integration-test scaffolding.
- `handl-packages/` — `.hnl` packages and contracts; `data/` — YAML language fragments.
- `docs/` — moved to the sibling **`docs`** repo (the VitePress hub): the site,
  the published ADR index, and the aggregated per-repo reference/rustdoc.
- `project/` — project notes and ADRs.

## Current status

- The pluggable expression pipeline and canonical kit are functional: lexing, parsing, error recovery, statement modules, formatting, and diagnostics all have working tests.
- `handl-kit-canonical` ships with optional typecheck and runtime plugin slots; the default `CanonicalKit` loads the canonical parser but does not enable full typecheck/runtime implementations unless they are configured.
- `handl-tooling` / `handl-host-lsp` (latter in the `lsp` repo) provide LSP plumbing with decoupled completion, goto-definition, document-symbols, and incremental-sync plugin slots. Hover and real source-position resolution are still placeholders.
- Many compiler-pass, capability, and VM crates are partially implemented or still stubs; the workspace is in active construction.

See `.devin/AGENTS.md` for commands. Language and runtime details live in the
sibling `docs` repo (`LANG.md`, `RUNTIME.md`, `PAPER.md`, `SAE.md`); project
notes and ADRs are on the `project` branch, mounted at `project/` (ADR-0129).
