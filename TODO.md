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
- [x] `hnl fmt --check && hnl lint && hnl typecheck && hnl test` CI gate for
  `handl-packages/` and `examples/` (workflow + script added). **The gate is
  GREEN and blocking**: `continue-on-error` removed from
  `.github/workflows/dogfood-gate.yml`, script in `scripts/dogfood-gate.sh`.
  Over the 177-file corpus: fmt 0 failures, lint 0 errors (residual
  `missing-module-doc`/`naming-convention` info+warnings only), typecheck 0
  errors, test 43/43 pass, 0 parse-error files. See `AGENTS.md`
  § "Dogfood gate status — GREEN" for the infrastructure that made it green
  (`split_statements`, `detect_dialect`, `check_module_with_prelude`,
  `canonical_wire_names`).

## Engine genericity and known issues

See `AGENTS.md` § "Known issues" for the source of truth. These are the
residual hard-codings that break the "language as data" invariant and the
current failing/warning tests.

**Status: no open failing or warning items.** The engine-genericity debt is
closed except for the sites listed below, which are blocked on adding new
`Language` vocabulary (a language-design decision, not a sweep); the design
write-up lives in `project/tasks/0081-remaining-kind-dispatch.md`.

- [~] Engine-genericity debt: the *structural-token* residuals in
      `plugin-engine-parse-pratt` (block opener, dimension bracket, block
      separators, keyword-as-identifier), the positional `field-access`/
      `type-annot` reads in `plugin-engine-transform-rewrite`'s
      `desugar.data`, the `token.identifier` probes in `handl-runtime`, and
      the duplicated `kind: export` lookups in `plugin-lsp` are **closed** —
      they resolve through `Language` token data (`token_kind_for_label` /
      `token_in_category` / `token_pair`) and `child_role_index`, with
      canonical fallbacks. See `AGENTS.md` §"Engine-genericity residual
      audit" (a2) for the full per-site list.
      What remains is blocked on **missing vocabulary**, not effort:
      `plugin-ext-refinement/src/arrow_parser.rs` is documented bootstrap
      (the arrow combinator vocabulary is calculus-kernel syntax), and
      `handl-kit-arrow-lower` (`assign`/`bind`/`op`/`function`/`rec`/
      `var`/`call`) + `host-cli/transform_categorical.rs` (`dim_arrow`/
      `stmt.import`/`var`/`value`) name specific canonical constructs.
      Closing those needs a new `Language` field (e.g. `binding_kind` /
      `arrow_kind` / `pattern_role: tag`) — a language-design decision, not
      a sweep fix.
- [x] `handl-defining-handl` cranelift bootstrap test fails on `let` bindings
      (internal `let`/`stmt.let` kinds — no `let` surface syntax) —
      **closed**: no `let`/`stmt.let` reference remains anywhere in the crate,
      and the whole `handl-defining-handl` suite is green (verified:
      `bootstrap_self` 5/5, `bootstrap` 2/2, `bootstrap_host_fn` 2/2, plus the
      parity suites; `bootstrap_proc`'s single case is `#\[ignore\]`d for the
      removed `|>`/`~>` surface syntax, not `let`). The entry was stale — the
      `let` removal (ADR-0079) and the corpus rewrite to `block`+`assign`
      already fixed it.
- [x] `plugin-engine-parse-pratt` unused import warning (`PayloadTokenSpec`) —
      **closed**: the symbol was already gone when the structural-token pass
      touched the crate; no warning is emitted. (Was stale.)

## High priority — bootstrap tooling integration (ADR-0085..0094)

Phase 1 crates are now `plugin-ext-diag`, `plugin-ext-fmt`, `plugin-ext-lint`,
`plugin-engine-typecheck` (+ `plugin-engine-typecheck-nbe`), `plugin-engine-test`,
and `host-cli` / `plugin-ext-cli`. The CLI subcommands are `fmt`, `lint`,
`typecheck` (not `check`), `test`, `run`, `eval`, `transform`, `gen-grammar`.

- [~] Wire `plugin-ext-fmt` / `plugin-ext-lint` / `plugin-engine-typecheck` /
      `plugin-engine-test` through the existing `plugin-ext-*` capability
      contracts so `host-lsp`, `host-cli`, and third parties can reuse them.
      **Scoped, not started** — see
      `project/tasks/0085-tooling-capability-wiring.md`. The item reads like
      wiring; it is not. None of `plugin-ext-{fmt,lint,eval,build,compile}`
      declares a `Plugin`/`Behavior` at all (they are free-function modules),
      so the plugins must be **created** first. `host-cli` already links
      `DefaultHost` + `BasicCli` but calls the free functions directly.
      Steps 1–3 (add a thin adapter `Plugin` per tool, decide its payload
      schema, register it) are additive and are the natural first PR;
      step 4 (reroute `host-cli`'s subcommand bodies) touches the primary
      user-facing binary and should follow once the plugins are individually
      tested.
- [ ] Bring the linter / typechecker rule engines onto the generic
      `plugin-engine-*` machinery where applicable
- [ ] Replace any pre-arrow tooling duplicates with the improved
      implementations, keeping the generic contracts
- [ ] 100% coverage gate for the tooling plugin crates in CI;
      `cargo test --workspace` green
- [ ] Update the README crate map to reflect the improved tooling set
- [x] `plugin-engine-test`: emit `SuiteStarted`/`SuiteFinished` and add
      structured `expected`/`actual`/`diff` to `TestOutcome`. **Landed** (ADR-0089):
      suites bracket each module; `FailureDetail` carries
      `expected`/`actual`/`diff` from both the engine `assert` path
      (`TestAssertHandler` thread-local) and the pure-handl `assert.*` helpers'
      `err` payload; both `HumanReporter` and `JsonReporter` render them
      (human `== <module> ==` headers, json `suites` array). See `AGENTS.md`
      § "ADR-0089 prerequisite (done)". Note `diff` is still only produced by
      the engine `assert` path — the `.hnl` helpers don't emit one yet.

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
