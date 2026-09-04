# TODO

## High priority — bootstrap tooling (ADR-0085..0090)

Temporary, surface-syntax-coupled Rust tooling so the stdlib can finally be
written in handl with real fmt/lint/check/test. Full plan in
[ADR-0085](adrs/0085-bootstrap-tooling-strategy.md).

### Phase 0 — decisions land
- [ ] Accept ADR-0085 (bootstrap tooling strategy) and ADR-0086..0090
- [ ] Pin the v0.1 surface grammar/AST versions the tooling builds against
      (ADR-0074 + `ARROW_GRAMMAR_V0_1.md`); CI gate on grammar drift

### Phase 1 — crates
- [ ] `crates/tooling-fmt` (ADR-0086): `format_source`, golden corpus per
      ADR-0074 construct, idempotence property test, 100% coverage
- [ ] `crates/tooling-linter` (ADR-0087): Diagnostic/Span/Fix types, rules/
      module, initial rule set (`unused-import`, `unbound-wire`,
      `shadowed-binding`, `dead-branch`, `naming-convention`,
      `missing-module-doc`, `fmt`), 100% coverage
- [ ] `crates/tooling-typechecker` (ADR-0088): arrow kind checking,
      structural inference, `rec` region well-formedness, module-level
      checks; explicit "not yet checked" boundaries for row vars/traits
- [ ] `crates/tooling-test-runner` (ADR-0089): `test` declaration discovery,
      eval-engine execution, assertions via stdlib, `--filter/--list/--json`,
      exit codes
- [ ] `crates/tooling-cli` (ADR-0090): `hnl` binary —
      `fmt/lint/check/test/run/eval`; direct deps on all of the above; no
      plugin system by design

### Phase 2 — dogfood
- [ ] Write the handl stdlib in handl (resumes ADR-0077 work) with
      `hnl fmt --check && hnl lint && hnl check && hnl test` as a required
      CI gate
- [ ] Migrate `handl-packages/` and `examples/` to clean fmt/lint/check/test

### Phase 3 — legacy crate deletion (ADR-0085 rule 4)
Audit each before deleting; delete in the same PR that removes its last
consumer. Update the README crate map with every deletion.
- [ ] Delete `plugin-ext-fmt`, `plugin-ext-lint`, `plugin-ext-typecheck`,
      `plugin-ext-refinement`, `plugin-ext-compile`, `plugin-ext-eval`,
      `plugin-ext-build`, `plugin-ext-kit`, `plugin-ext-plugin`,
      `plugin-ext-cli` (capability plumbing superseded by direct tool
      crates)
- [ ] Delete `plugin-exec-*` (exec policy plugins — pre-arrow, unused by
      tooling)
- [ ] Delete `plugin-lsp-completion`, `plugin-lsp-goto`, `plugin-lsp-symbol`,
      `host-lsp`, `host-tooling` (LSP deferred to RFC-0006; keep `adrs/`
      ADR-LSP as the record)
- [ ] Delete `host-cli` once `hnl` (tooling-cli) reaches parity for stdlib
      workflow commands
- [ ] Audit remaining `kit-*` / `plugin-*` crates for arrow-path usage;
      delete or explicitly re-home (moved under `tooling-*` needs)
- [ ] Regenerate README crate directory map; workspace `Cargo.toml` members
      pruned to the end-state set: `platform`, `handl-*` core,
      `plugin-engine-*`, `plugin-lib-*`, five `tooling-*` crates

### Phase 4 — exit (low priority, see project/rfcs/ + BACKLOG.md)
Self-host each tool per its RFC and delete its bootstrap crate:
fmt → RFC-0002, lint → RFC-0003, typecheck → RFC-0004, test → RFC-0005,
LSP → RFC-0006, CLI+plugin system → RFC-0007/0001.

## High priority — package manager

`import` grammar and eval scheme exist (`grammar.stmt-import`, `eval.stmt-import`)
but no engine implements the `import` op and there is no module loader in the
runtime. `.handl.toml` has no dependency resolution. Without this, programs
cannot import stdlib modules or split across files.

- [ ] Implement `import` op in all three eval engines (tree-walk, bytecode, cranelift)
- [ ] Add module loader to `handl-runtime` (`load_module`/`resolve_module`)
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

