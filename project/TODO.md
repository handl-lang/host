# TODO

## High priority — package manager and `import`

The `import` op is now implemented in all three eval engines and the runtime
has a module loader. The remaining blockers are dependency resolution in
`.handl.toml` and a clear module-resolution policy.

- [x] Implement `import` op in tree-walk engine
- [x] Implement `import` op in bytecode engine
- [x] Implement `import` op in cranelift engine
- [x] Add module loader to `handl-runtime` (`load_module`/`resolve_import`)
- [ ] Define module resolution semantics (path-based? registry-based?)
- [ ] Add dependency resolution to `.handl.toml` (`[dependencies]` section)
- [ ] Wire `import` through the desugar pass if needed
- [ ] Test: `import "arrows/fn/map.hnl"` works end-to-end

## High priority — handl stdlib in handl (ADR-0092)

`plugin-lib-test` is deleted; the test runner is now `plugin-engine-test`
(ADR-0089). `error.raise`/`error.try` and the clock/RNG primitives the
framework needs are wired. The remaining blocker is moving the framework's
`handl "..."` bindings to pure-handl source files.

- [ ] Rewrite the `handl "..."` bindings in `plugin-engine-test::framework` as
      pure-handl `assert.hnl`/`property.hnl`/etc. (per ADR-0092 target layout)
- [ ] `hnl fmt --check && hnl lint && hnl typecheck && hnl test` CI gate for
      `handl-packages/` and `examples/`

## High priority — engine genericity and known issues

See `AGENTS.md` § "Known issues" for the source of truth. These are the
residual hard-codings that break the "language as data" invariant and the
current failing/warning tests.

- [ ] Engine-genericity debt: close remaining `kind ==` / positional-child
      hits across `plugin-engine-parse-pratt`, `plugin-engine-transform-rewrite`,
      `host-lsp`, and `plugin-ext-refinement`
- [ ] `handl-defining-handl` cranelift bootstrap test fails on `let` bindings
      (internal `let`/`stmt.let` kinds — no `let` surface syntax)
- [ ] `plugin-engine-parse-pratt` unused import warning (`PayloadTokenSpec`)

## High priority — bootstrap tooling integration (ADR-0085..0094)

Phase 1 crates are now `plugin-ext-diag`, `plugin-ext-fmt`, `plugin-ext-lint`,
`plugin-engine-typecheck` (+ `plugin-engine-typecheck-nbe`), `plugin-engine-test`,
and `host-cli` / `plugin-ext-cli`. The CLI subcommands are `fmt`, `lint`,
`typecheck` (not `check`), `test`, `run`, `eval`, `transform`, `gen-grammar`.

- [ ] Wire `plugin-ext-fmt` / `plugin-ext-lint` / `plugin-engine-typecheck` /
      `plugin-engine-test` through the existing `plugin-ext-*` capability
      contracts so `host-lsp`, `host-cli`, and third parties can reuse them
- [ ] Bring the linter / typechecker rule engines onto the generic
      `plugin-engine-*` machinery where applicable
- [ ] Replace any pre-arrow tooling duplicates with the improved
      implementations, keeping the generic contracts
- [ ] 100% coverage gate for the tooling plugin crates in CI;
      `cargo test --workspace` green
- [ ] Update the README crate map to reflect the improved tooling set
- [ ] `plugin-engine-test`: emit `SuiteStarted`/`SuiteFinished` and add
      structured `expected`/`actual`/`diff` to `TestOutcome`

## Unblocking — test/perf infrastructure

These are still open and should be picked up alongside the above.

- [ ] Restore bench suite (core, infer, lexer, parser, pipeline, typecheck)
- [ ] Restore integration tests (fuzz, generics, integration, mock)
- [ ] Restore platform/e2e tests (e2e, language_host, integration, mock)
- [ ] Restore ext-tests and compiler-tests after compiler/typecheck refactor

## Recently completed

- `plugin-lib-test` deleted; test runner is now `crates/plugin-engine-test`
  (ADR-0089) and `hnl test` is wired in `crates/host-cli/src/main.rs`
  (`--filter`, `--list`, `--report`, `--jobs`, `--cache`, `--tag`, `--changed`).
- `hnl check` renamed to `hnl typecheck`.
- `plugin-typecheck-core`, `plugin-typecheck-nbe`, `plugin-ext-typecheck`,
  `kit-type*` deleted; typecheck engines are `plugin-engine-typecheck` and
  `plugin-engine-typecheck-nbe`, selected by `runtime.typecheck: nbe` in
  `crates/handl-kit-data-canonical/data/kits/canonical.yaml`.
- `plugin-ext-diag`, `plugin-ext-fmt`, `plugin-ext-lint`, `host-lsp`,
  `plugin-engine-test` exist and are wired.
- `handl-kit-data-config` deleted; kit manifests live in
  `crates/handl-kit-data-canonical/data/kits/*.yaml`.
- `import` op is implemented in all three eval engines.
