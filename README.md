# handl platform

`handl` is a small, embeddable, runtime-agnostic expression language and a pluggable language construction kit. This repository is the reference Rust platform: a Cargo workspace where the canonical syntax is assembled from tiny, swappable plugin crates (lexical rules, parser fragments, statements, formatting, diagnostics, LSP, runtime, and typecheck). The core contract lives in `handl-platform`, and everything from the canonical kit to the LSP host builds on top of it.

## Computational model

**Arrow calculus, not lambda calculus.** The kernel (ADR-0052), grounded by [ADR-0106](project/adrs/0106-object-arrow-kernel.md), is six arrow-calculus primitives: `arr`/`>>>`/`first`/`choice`/`loop`/`app`. No lambda, no lambda application. `proc` desugars to arrow combinators; calling a `proc` is `app` (dynamic arrow invocation).

**One object world.** Everything is an object (`Obj : sym()`); the arrow `A ~> B` is the only computational primitive. `sym()` / `sym(A)` mint nominal identity, and the same `sym` parent graph powers `instanceof`, `<:`, and polymorphism — there is no separate type, function, effect, or proof universe.

**Declarative, not imperative.** No mutation primitive — reassignment was deleted from the kernel. Binding (`x = y`) is immutable `Seq` with no lexical environment to extend (see ADR-0106). State goes through `loop` (feedback) or explicit threading (`State ~> (State, Result)`), not variable mutation.

**Descriptions, not execution.** `>>>` composes descriptions without running them. Handlers interpret the same description differently: runtime runs it, compiler lowers it, analyzer inspects it.

## High-level architecture

- `crates/platform` — core contracts: `Capability`/`CapabilityId`, `CapabilityRequest`/`CapabilityResponse`, `Plugin`, `PluginRegistry`, `Kit`, `KitRegistry`, `DefaultHost`, `PlatformError`.
- `crates/handl-language` — `Language`, `Ast`, `AstNode`, `AstRef`, and data-driven shape/type/eval/grammar fragments. **Moved to the sibling `handl-lang/kernel` repo** (with `handl-value`, `handl-ext`, `handl-defining-handl`, and the `handl-lib-*` substrate packages; `hnl-engine` lives in `handl-lang/engine`); consumed here as a `main`-floating git dep patched to `../kernel` for local dev.
- `crates/handl-kit-*` — canonical language data assembly and ready-made kit loading (`kit-canonical` lives in `handl-lang/kit-canonical`).
- `crates/handl-runtime` — runtime coordination, desugaring, and eval-engine dispatch.
- `crates/plugin-ext-*` — capability-contract surfaces and default rule-acceptor plugins: `diag`, `detect`, `fmt`, `lint`, `kit`, `eval`, `refinement`, `compile`, `plugin`, `sdkgen`, `test`, `typecheck`. The `hnl` *commands* they back live in the `ext-*` repos as dynamic cdylibs (ADR-0149); `cli`/`lsp` moved to their host repos in ADR-0128 Phase 8.
- `hnl-engine` / `hnl-engine-*` — the engine substrate and concrete tooling engines (`lex`, `parse`, `fmt`, `lint`, `typecheck`, `test`, `transform`, `exec`, …) live in the sibling **`engine*`** repos; consumed as `main`-floating git deps.
- `crates/plugin-lib-*` — standard-library-style native/runtime modules (`stdlib`, `handl`, `scheduler`, `tag`; the `lib-*` ports live in their own repos).
- `crates/plugin-applier-*` / `plugin-load*` — composition resolution (`applier`, `applier-bootstrap`, `applier-default`) and the plugin loaders (`plugin-load`, `plugin-loader-native`).
- `crates/host-*` — `host-tooling` (semantic tokens and LSP helpers); the end-user hosts moved out in ADR-0128 Phase 8 — `host-cli` (`hnl`) lives in the sibling **`host-cli`** repo, `host-lsp` (LSP server) in **`host-lsp`**.
- `crates/handl-value` — shared value model. **Moved to `handl-lang/kernel`.** `handl-test-utils` — the conformance corpus; `handl-defining-handl` moved to `handl-lang/kernel` (`crates/defining-handl`).
- `handl-packages/` — generated `.hnl` bindings (sdkgen output). The
  canonical language data (YAML fragments) and the `.hnl` packages live
  in the `kit-*` repos (`kit-prelude-lang`, …).

## Build and test

```bash
cargo check --workspace
cargo test --workspace
# `hnl` lives in the sibling `host-cli` repo (ADR-0128 Phase 8, renamed in ADR-0149):
(cd ../host-cli && cargo build -p handl-host-cli)   # produces ../host-cli/target/debug/hnl
```

## The `hnl` CLI

The `hnl` binary lives in the sibling **`host-cli`** repo. Since
ADR-0149 it is a **dumb host**: it discovers extension shared libraries
(`libhnl_ext_*.so`), reads their JSON manifests, and dispatches argv to
them — it contains no tool code itself. Commands are provided by the
`ext-*` repos (each builds a cdylib): `ext-fmt` → `fmt`,
`ext-lint` → `lint`, `ext-typecheck` → `typecheck`/`check`,
`ext-eval` → `run`/`eval`, `ext-test` → `test`, `ext-compile` → `compile`,
`ext-sdkgen` → `sdkgen`, `ext-transform` → `transform`,
`ext-kit` → `kit`/`gen-grammar`, `ext-plugin` → `plugin`.

Discovery: `--ext-dir <dir>` (repeatable) → `$HANDL_EXT_DIR` (PATH-style
list) → `~/.handl/ext/`; debug builds also scan `<exe-dir>/ext/` and
`<exe-dir>/`. `hnl ext list`/`hnl ext doctor` report the registry.

For development, build the extensions you need and point the host at
their target dirs:

```bash
(cd ../ext-typecheck && cargo build) && (cd ../ext-lint && cargo build)
HANDL_EXT_DIR=../ext-typecheck/target/debug:../ext-lint/target/debug \
  ../host-cli/target/debug/hnl typecheck file.hnl
```

Exit codes: `0` success, `1` tool-reported failure, `2` usage or parse
error; unknown command exits `2` listing the loaded commands.

## Crate directory map

- `crates/handl-*` — language core: `handl-kit-data`, `handl-kit-arrow-lower`, `handl-runtime`, `handl-test-utils`. (`handl-language`, `handl-value`, `handl-defining-handl` and the `handl-lib-*` packages moved to **`kernel`**; `kit-canonical` to **`kit-canonical`**.)
- `crates/platform` — core platform contract (`Plugin`, `Capability`, `Host`, `Kit`); `crates/platform-tests` — its integration tests.
- `hnl-engine` / `crates/hnl-engine-*` — the generic engine substrate and concrete engines, all data-driven off `Language`, moved to the sibling **`engine*`** repos:
  - front end — `lex`, `lex-pattern`, `parse`, `parse-pratt`
  - tooling — `fmt`, `lint`, `typecheck` (NBE engine + bootstrap scope checker), `test`
  - transforms — `transform`, `transform-rewrite`
  - execution — `exec`, `exec-tree-walk`, `exec-bytecode`, `exec-cranelift`
- `crates/plugin-ext-*` — capability contracts and the default provider per category (`diag`, `detect`, `fmt`, `lint`, `typecheck`, `test`, `eval`, `refinement`, `compile`, `kit`, `plugin`, `sdkgen`; `cli`/`lsp` moved to their host repos). The tooling four (`fmt`/`lint`/`typecheck`/`test`) are thin `Plugin` adapters over their `hnl-engine-*` engine — see ADR-0085.
- `crates/plugin-exec-cache` / `plugin-compile` — the exec result cache and the compiler contribution point; the back ends (`dot`, `js`, `llvm`, `sexp`, `term`, `handl`) live in the **`compile-*`** repos.
- `crates/plugin-sdkgen` — the SDK generator contract; its back ends/emitters live in the **`sdkgen-*`** repos.
- `crates/plugin-lib-*` — standard library, native ops, and utility libraries.
- `crates/plugin-applier-*` / `plugin-load*` / `plugin-loader-native` — applier composition and the native loader; `plugin-module` — module plumbing.
- `crates/host-*` — `host-tooling` (the shared tooling/LSP host contract). `host-cli` + `plugin-ext-cli` moved to the **`host-cli`** repo; `host-lsp` + `plugin-lsp` + `plugin-lsp-std` + `plugin-ext-lsp` moved to the **`host-lsp`** repo (ADR-0128 Phase 8, repos renamed ADR-0149).
- `crates/bench` / `ext-tests` — criterion benchmarks and integration-test scaffolding.
- `handl-packages/` — generated `.hnl` bindings (sdkgen output); canonical language data and `.hnl` packages live in the **`kit-*`** repos.
- `docs/site/` — the published Handl product pages, vendored by the sibling **`docs`** repo (the VitePress hub) to `/handl/`: the site,
  the published ADR index, and the aggregated per-repo reference/rustdoc.
- `project/` — project notes and ADRs.

## Current status

- The pluggable expression pipeline and canonical kit are functional: lexing, parsing, error recovery, statement modules, formatting, and diagnostics all have working tests.
- `handl-kit-canonical` (in the **`kit-canonical`** repo) ships with optional typecheck and runtime plugin slots; the default `CanonicalKit` loads the canonical parser but does not enable full typecheck/runtime implementations unless they are configured.
- `handl-host-tooling` / `handl-host-lsp` (latter in the `host-lsp` repo) provide LSP plumbing with decoupled completion, goto-definition, document-symbols, and incremental-sync plugin slots. Hover and real source-position resolution are still placeholders.
- Many compiler-pass, capability, and VM crates are partially implemented or still stubs; the workspace is in active construction.

See `AGENTS.md`/`.devin/AGENTS.md` for commands. Language and runtime details
live in `docs/site/` (`PAPER.md`, `SAE.md`, `explanation/`), vendored by the
sibling `docs` repo to `/handl/`; project
notes and ADRs are on the `project` branch, mounted at `project/` (ADR-0129).
