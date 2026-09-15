# TODO

## High priority — package manager and `import`

The `import` op is now implemented in all three eval engines and the runtime
has a module loader. The remaining blockers are dependency resolution in
`.handl.toml` and a clear module-resolution policy.

- [x] Implement `import` op in tree-walk engine
- [x] Implement `import` op in bytecode engine
- [x] Implement `import` op in cranelift engine
- [x] Add module loader to `handl-runtime` (`load_module`/`resolve_import`)
- [x] Define module resolution semantics — path-based: `[dependencies]`
      specifier → package dir, else module-dir/module-root/CWD-relative
      (ADR-0095; registry/content-addressed resolution stays future work
      per ADR-0010)
- [x] Add dependency resolution to `.handl.toml` (`[dependencies]` section
      in `handl-platform::config::HostConfig`; vendored/path deps only)
- [x] Wire `import` through the desugar pass — verified not needed:
      `stmt.import` has no desugar rule and passes through unchanged;
      `ModuleLoader` already applies `desugar` to imported modules
- [x] Test: `import "arrows/fn/map.hnl"` works end-to-end
      (`kit-canonical/tests/import_export.rs`; `hnl run`/`hnl eval`
      verified against a fixture project)

## High priority — handl stdlib in handl (ADR-0092)

`plugin-lib-test` is deleted; the test runner is now `plugin-engine-test`
(ADR-0089). The framework is now pure handl: `handl-packages/test/` holds
`assert.hnl`/`property.hnl`/`benchmark.hnl`/`fuzz.hnl`/`snapshot.hnl`/
`test.hnl` and the canonical kit registers them. The pure-handl ops use the
result pattern (`ok`/`err` tagged values) and do not rely on `error.raise`/
`error.try`; the runner's per-case `EvalError` → failed `TestOutcome` path
remains unchanged.

- [x] Rewrite the temporary `plugin-engine-test::framework` Rust handls as
      pure-handl `assert.hnl`/`property.hnl`/etc. (per ADR-0092 target layout)
- [~] `hnl fmt --check && hnl lint && hnl typecheck && hnl test` CI gate for
  `handl-packages/` and `examples/` (workflow + script added; currently
  informational/continue-on-error because the gate is not green: fmt 1/172
  canonical, lint 139/172 files with diagnostics, typecheck 141/172 files
  with diagnostics, test 42 pass but 32 scanned modules have parse errors)

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
- Module resolution semantics + `.handl.toml` `[dependencies]` landed
  (ADR-0095): `import "arrows/fn/map.hnl"` works end-to-end through
  `hnl run`/`hnl eval`, including nested relative imports and
  cross-module arrow values via `EvalEngine::seal_module_value` +
  `__home__` frame stamping. `from "<mod>" import` module-name forms are
  still handled by the `HandlModule`/`ModuleResolver` seam (and are still
  stripped by `hnl run`), not the file loader.
