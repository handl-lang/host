# TODO

## High priority — bootstrap tooling (ADR-0085..0090)

Temporary, surface-syntax-coupled Rust tooling so the stdlib can finally be
written in handl with real fmt/lint/check/test. Full plan in
[ADR-0085](adrs/0085-bootstrap-tooling-strategy.md).

### Phase 0 — decisions land

- [ ] Accept ADR-0085 (bootstrap tooling strategy) and ADR-0086..0091
- [ ] Pin the v0.1 surface grammar/AST versions the tooling builds against
      (ADR-0074 + `ARROW_GRAMMAR_V0_1.md`); CI gate on grammar drift

### Phase 1 — improve the plugin crates (no new tooling crates)

- [ ] `crates/plugin-ext-diag` (ADR-0091, do first): shared
      `Diagnostic`/`Severity`/`Span`/`Fix` types + shared renderer; migrate
      `plugin-ext-fmt`, `plugin-ext-lint`, `plugin-engine-typecheck`,
      `plugin-lib-test`, and `host-cli` onto them; `plugin-diag-expression`
      switches to the shared types
- [ ] `crates/plugin-ext-fmt` (ADR-0086): `format_source`, golden corpus per
      ADR-0074 construct, idempotence property test, 100% coverage
- [ ] `crates/plugin-ext-lint` (ADR-0087): rules/ module over the generic
      registry, initial rule set (`unused-import`, `unbound-wire`,
      `shadowed-binding`, `dead-branch`, `naming-convention`,
      `missing-module-doc`, `fmt`), 100% coverage
- [ ] `crates/plugin-engine-typecheck` (+ `plugin-typecheck-core/-nbe`)
      (ADR-0088): arrow kind checking, structural inference, `rec` region
      well-formedness, module-level checks; explicit "not yet checked"
      boundaries for row vars/traits
- [ ] `crates/plugin-lib-test` (ADR-0089): `test` declaration discovery,
      eval-engine execution, assertions via stdlib, `--filter/--list/--json`,
      exit codes
- [ ] `crates/host-cli` + `crates/plugin-ext-cli` (ADR-0090): `hnl`
      subcommands — `fmt/lint/check/test/run/eval`; dispatch through the
      existing `plugin-ext-*` capability contracts

### Phase 2 — dogfood

- [ ] Write the handl stdlib in handl (resumes ADR-0077 work) with
      `hnl fmt --check && hnl lint && hnl check && hnl test` as a required
      CI gate
- [ ] Migrate `handl-packages/` and `examples/` to clean fmt/lint/check/test

### Phase 3 — integrate on the substrate (no deletions)

The Rust substrate stays (ADR-0085 rule 1): the plugin system,
`plugin-ext-*`, and `plugin-engine-*` are kept, tested, and improved —
they modularize the Rust code and are what the future handl port moves.

- [ ] Wire `plugin-ext-fmt`/`plugin-ext-lint`/`plugin-engine-typecheck`/
      `plugin-lib-test` through the existing `plugin-ext-*` capability
      contracts so `host-lsp`, `host-cli`, and third parties can reuse them
- [ ] Bring the linter/typechecker rule engines onto the generic
      `plugin-engine-*` machinery where applicable (parse, typecheck)
- [ ] Replace any pre-arrow tooling duplicates (e.g. old fmt/lint stubs)
      with the improved implementations, keeping the generic contracts
- [ ] 100% coverage gate for the tooling plugin crates in CI;
      `cargo test --workspace` green; bench suite restored against the tools
- [ ] Update the README crate map to reflect the improved tooling set

### Phase 4 — exit (low priority, see project/rfcs/ + BACKLOG.md)

Port each tool to handl per its RFC — a _move_ of already-generic, tested
Rust designs, not a rewrite; the Rust substrate remains as bootstrap and
fallback:
fmt → RFC-0002, lint → RFC-0003, typecheck → RFC-0004, test → RFC-0005,
LSP → RFC-0006, CLI+plugin system → RFC-0007/0001.

## High priority — package manager

`import` grammar and eval scheme exist (`grammar.stmt-import`, `eval.stmt-import`).
The `import` op is implemented in the tree-walk and bytecode engines; the
runtime has a `ModuleLoader` (`load_module`/`resolve_import`). `.handl.toml` has
no dependency resolution yet. Without the remaining pieces, programs cannot
import stdlib modules or split across files.

- [x] Implement `import` op in tree-walk engine
- [x] Implement `import` op in bytecode engine
- [ ] Implement `import` op in cranelift engine
- [x] Add module loader to `handl-runtime` (`load_module`/`resolve_import`)
- [ ] Define module resolution semantics (path-based? registry-based?)
- [ ] Add dependency resolution to `.handl.toml` (`[dependencies]` section)
- [ ] Wire `import` through the desugar pass if needed
- [ ] Test: `import "arrows/fn/map.hnl"` works end-to-end

## Unblocking (still open, do before/alongside Phase 2)

- [ ] Add LSP host (Rust `host-lsp` parity — editor support now; handl LSP
      is RFC-0006)
- [ ] Add vscode extension
- [ ] Restore bench suite (core, infer, lexer, parser, pipeline, typecheck)
- [ ] Restore integration tests (fuzz, generics, integration, mock)
- [ ] Restore platform/e2e tests (e2e, language_host, integration, mock)
- [ ] Restore ext-tests and compiler-tests after compiler/typecheck refactor
