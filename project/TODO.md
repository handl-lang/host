# TODO

## Done — wrongly-placed Rust reifications retired (ADR-0040 kernel rule)

Rust holds the kernel (`Plugin`, `Host`, `Operation`, `Behavior`,
`RuleAcceptor`, `Contribution` — ADR-0040) plus generic tooling; everything
else is Handl over a C ABI / SDK-gen contract. A dead-code sweep (static
`cargo check --all-targets` plus `llvm-cov` over the whole workspace) found 15
crates that nothing live depends on — ideas reified as Rust crates no binary
ever linked. They are retired, not deleted from history.

- [x] **Idea ledger + gate.** `project/parked.yaml` records each retired idea
      (adr, `status`, `superseded_by`, `unpark` condition). `crates/platform-tests/
      tests/wiring.rs` fails the build if a retired crate is still a member, if a
      member is an undeclared orphan, or if the ledger goes stale. Removal is
      reversible — `git tag parked/<family>-2026` marks the commit before each
      removal, so `git checkout <tag> -- crates/<dir>` restores it.
- [x] **Retired (14 crates, ~2.9k LOC):** the unwired `plugin-exec-*` family
      (`allow`/`deny`/`counter`/`map`/`trace`/`max-calls`/`max-depth`/`logging`
      — the ADR-0040 limits are already inlined in `SandboxHost`),
      `plugin-loader-wasm`/`plugin-loader-web` (ADR-0035, never accepted),
      `plugin-ext-build` (3-LOC placeholder), `plugin-engine-transform-parallel`
      (ADR-0094; the sequential `RewriteEngine` is the live path),
      `plugin-diag-expression` (superseded by `plugin-ext-diag` +
      lint-rules-as-data), and `plugin-source` (spans now ride on the AST).
      `plugin-exec-cache` is unaffected — it is live via `plugin-engine-test`.
- [x] **Vestigial `KitManifest`.** `kit-canonical`'s duplicate `KitManifest` +
      `canonical_manifest()` existed only to feed `Kit::from_manifest`, a compat
      shim that ignored its argument ("the manifest is replaced by the data
      fragment registry"). All three deleted.

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

- [x] Wire `plugin-ext-fmt` / `plugin-ext-lint` / `plugin-engine-typecheck` /
      `plugin-engine-test` through the existing `plugin-ext-*` capability
      contracts so `host-lsp`, `host-cli`, and third parties can reuse them.
      **Landed** — see `project/tasks/0085-tooling-capability-wiring.md`.
      `FmtPlugin`, `LintPlugin`, `TypecheckPlugin`, and `TestPlugin` all exist,
      are registered in `host-cli`'s `tooling_host()`, and `hnl
      fmt`/`lint`/`typecheck`/`test` all dispatch through them (lossless: same
      stdout/stderr routing and exit codes). `lint` carries the caller's
      serialized `Language` (ADR-0124); `typecheck`/`test` do not need it —
      both sides assemble from the single `Kit::new` path, so there is no
      caller-vs-plugin divergence to close. `host-lsp` is *not* converted yet.
- [x] Assess converting `host-lsp` to the capability path — **decision: don't,
      as a rewrite**. `host-lsp` is a 94-file / ~14k-line LSP *protocol server*
      (long-lived loop, incremental document store, per-request cancellation,
      workspace-wide rename/symbols), not a one-shot capability
      request/response; the `fmt`/`lint`/`typecheck`/`test` capability shape
      does not fit it. It also already sits on the plugin architecture
      (`ToolingHost` + `plugin-lsp::BasicLsp` providers), so it is not
      bypassing the contracts. The reusable value the ADR-0085 work was after —
      a shared fmt/lint/typecheck implementation — is already reachable from
      `host-lsp` through those same crates; re-plumbing ~14k lines for no
      functional gain is pure risk. Revisit only if a *capability-shaped* LSP
      request appears (e.g. serverless/one-shot formatting in a served
      context).
- [x] Bring the linter / typechecker rule engines onto the generic
      `plugin-engine-*` machinery where applicable — **verified already true**,
      no code change needed. Every tooling capability adapter is a thin
      `Plugin` over its engine: `plugin-ext-fmt` → `plugin-engine-fmt`
      (`FormatEngine`), `plugin-ext-lint` → `plugin-engine-lint`
      (`LintEngine` + `canonical_lint_natives`), `plugin-ext-test` →
      `plugin-engine-test`, and `plugin-ext-typecheck` → the kit's bound
      `plugin-engine-typecheck-handl` `HandlTypecheckEngine` (selected by
      `runtime.typecheck: nbe` in `data/kits/canonical.yaml`), which itself
      composes `plugin-engine-typecheck`'s bootstrap checker. The remaining
      hard-codings in those engines are the documented
      vocabulary-blocked ones (see the engine-genericity entry above).
- [x] Replace any pre-arrow tooling duplicates with the improved
      implementations, keeping the generic contracts — **landed**:
      `host-tooling` carried a private `Diagnostic`/`DiagnosticLevel` pair
      ("a local replacement for the former `handl_plugin_diag_core::Diagnostic`")
      plus hand-rolled `format`/`diagnose` pipelines that duplicated
      `plugin-ext-fmt` / `plugin-ext-lint`. `ToolingHost::format` and
      `::diagnose` now delegate to those crates and return the shared
      `plugin-ext-diag::Diagnostic` (rule name, line/col span, fix payload),
      so the third severity enum is gone. Five now-unused engine deps and two
      dead deps (`handl-plugin-load`, `handl-plugin-diag-expression`) were
      dropped. Its four `#[ignore]`d `perf.rs` tests (stale: "not yet
      implemented", and using the removed `|>` operator) were replaced by
      `tests/tooling.rs` — 10 behavioural delegation tests.
- [x] Coverage gate for the tooling plugin crates in CI; `cargo test
      --workspace` green — **landed** as `scripts/coverage.sh --tooling-gate`
      (blocking job `tooling-coverage-gate` in `coverage.yml`). Coverage went
      **90.32% → 99.64%** (144 → 7 uncovered lines) by adding ~40 tests: the
      `fmt`/`typecheck` adapters are now at **100%**, and the lint/test gaps
      are direct unit tests of the private degradation helpers. The 7
      residual lines are defensive guards that cannot execute on a 64-bit
      host (32-bit `usize` range check; fixpoint non-convergence; I/O-fault
      cache arms) — enumerated in `scripts/coverage.sh`. The literal "100%"
      is therefore not attainable without removing correct guards, so the
      gate is a 99% floor that still catches any real regression.
- [x] Update the README crate map to reflect the improved tooling set —
      **landed**: the map now lists the engine families by role (front end /
      tooling / transforms / execution), the `plugin-ext-*` providers with the
      tooling four called out as adapters, and the previously-missing
      `plugin-engine`, `plugin-exec-*`, `plugin-compile-*`, `plugin-sdkgen*`,
      `plugin-diag-expression`, `plugin-module`/`plugin-source`,
      `plugin-lsp*`, `bench`, `ext-tests`, and `platform-tests`.
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

- [~] **Extend the tooling coverage gate beyond the tooling adapters.**
      `plugin-ext-diag` is **done** (commit `9e1a1cda`): 97.85% -> 99.42% with 6
      targeted tests, and it is now in the gated set, which passes at 99.60%
      over five crates.
      The nine `plugin-exec-*` rule crates remain, and the assessment stands:
      they are 166-293 src lines each with only **2-3 unit tests** each, so a
      100% gate there means writing ~10 crates' worth of new tests — plan for
      that, it is not a config tweak. Measuring needs a `cargo llvm-cov`
      instrumented rebuild (~4-5 GB), so budget disk headroom:
      `cargo llvm-cov -p handl-plugin-exec-allow -p handl-plugin-exec-cache … --summary-only`.

- [ ] **The interpreted typecheck model is the remaining suite bottleneck.**
      After the loader fix above (64.5 min → 25 min), the leftover slow tests are
      all ones that reach the Handl typecheck model: `kit-canonical`'s
      `fp_integration` (worst single test **175 s**) / `ordered_annotations`
      (~121 s), and `plugin-engine-typecheck-handl::dependent_structure`
      (~40-60 s each at ~2 GB RSS).
      Measured inside `HandlTypecheckEngine::typecheck` on
      `data_typecheck_sees_desugared_block` (the 175 s case) — so the cost is
      neither marshalling nor loading:
      - `ensure_model` = **305 ns** (cached)
      - `marshal_language` (~31k nodes) = **6.2 ms**
      - `marshal_ast` = **0.2 ms**
      - model `func.call` = **the remaining ~175 s**
      The model (`typecheck_model.hnl`, ~51k chars) is evaluated by the tree-walk
      interpreter with a fixed fuel budget and O(n²) tendencies — as the code
      already says: "measured at minutes even for a 9-node AST, so it is
      unusable as a default CLI path". On these ASTs it burns the budget and
      then degrades to the Rust bootstrap checker anyway, i.e. the 175 s is
      pure waste.
      Root wiring: only `"handl"` (the model) is registered as a typecheck
      engine (`build_kit`), with `model_enabled` defaulting to **true**
      (`plugin-engine-typecheck-handl`, line 98), so *every*
      `kit.runtime().typecheck()` runs the model. The kit yaml's
      `runtime.typecheck: nbe` names an engine that no longer exists.
      Fix path (deliberate, cross-cutting — not rushed): register the runtime
      typecheck engine with `.with_model_enabled(false)` so
      `runtime().typecheck()` takes the fast bootstrap path, keep the model
      enabled only for the explicit `Kit::typecheck_handl*` entry points (which
      already own the model↔bootstrap fallback), and give `Kit::typecheck` the
      same bootstrap fallback on `NotImplemented`.
      **Two things checked on the way, so they need not be re-tried:**
      1. The fuel budget is **not** the lever. It is `fuel: 50000` in
         `project/design/adr0103/typecheck_model.hnl`, and 50000 ≈ 175 s, so
         lowering it to 5000 looked promising — but the 565-node `data Nat` AST
         still took >90 s. The cost is per-node model work, not fuel steps.
         (Exhaustion is graceful: it emits a `typecheck fuel exhausted`
         diagnostic and a `type.unknown`, it does not raise.)
      2. `.with_model_enabled(false)` on the *runtime* engine would break
         `Kit::typecheck`. Its unit tests assert model-produced types —
         `kit.typecheck("42").unwrap()` → `type.number`, plus `type.bool`,
         function types, `do { x :: 1; x }`, `match`, `x ~: type.string`
         mismatches, ~20 assertions in `kit-canonical`'s test module — and the
         bootstrap checker does **not** synthesize a type (the existing
         `NotImplemented` fallback in `typecheck_handl_with_prelude` returns
         `Value::Null`, not `report.typ`). So the wiring fix needs a
         source-level bootstrap *type synthesiser* behind the fallback first;
         without one it trades the slow tests for ~20 broken ones.
      Measured reference points: 3-node AST — model succeeds in **7.65 s**;
      565-node `data Nat` AST — model burns **175 s** and reports fuel
      exhaustion. So the model is only viable for very small ASTs, which is also
      what a tightened `MAX_MODEL_NODES` would encode (currently 1000, far above
      where the model stops being viable).

- [x] **A ~40 s one-time cost on the first module-backed eval — FIXED.**
      Cause: `kit-canonical`'s `build_kit` called
      `handl_plugin_lib_handl::install_loader(...)`, installing the
      self-hosted `.hnl` loader. `HandlLoader` runs the text transforms
      (`strip_comments`/`strip_decls`/`parse_imports`) as *interpreted handl*,
      so analysing one module executed **~21,000 interpreted `char`/`rev`/
      `lines` calls** (measured: `char.chars` ×2809, `len.len` ×2674,
      `head.head` ×2650, `rev.rev` ×2543 at ~2.6 ms each).
      `kit-canonical`'s own comment claimed it routed its modules around the
      loader via `with_loader_disabled`, but the install itself still made the
      first module analysis walk the handl pipeline.
      Fix: **don't install it** — the Rust `bootstrap` transforms are the
      "minimal active implementation" and `loader.hnl` is the "verified spec"
      of the same functions, with `plugin-lib-handl/tests/loader.rs` as the
      parity test. The seam stays available: a host can still call
      `install_loader` before building a kit.
      Measured effect (`cargo test -p handl-kit-canonical`, and note these are
      now one process per binary):
      - `tagged_values`: 42 s → **1.22 s**; `rand_c`: ~28 min → **7.65 s**;
        `intl_c`: ~11 min → **2.59 s**; `noise_c`: ~9 min → **2.84 s**.
      - Whole crate: ~45+ min at `-j 2` → **648 s single-threaded**, 252/252 pass.
      - CLI: first `hnl eval 'tag.tag(...)'` went from **>30 s** to milliseconds.
      Also confirmed while localising this: `Kit::new()` = 126 ms (my first
      guess blamed it, wrongly), `HandlModule::new` analysis = µs,
      `build_runtime` = 66 ms then sub-ms across 68 calls, `lower_ast` = 13 µs.
      Remaining slow spots in that crate are *not* this: the Handl-model
      typecheck tests (`fp_integration` ~8 s each, lib `typecheck_*`, and
      `ordered_annotations`) — a separate, still-open cost.
      Note `cargo test` (libtest, one process per binary) is also much better
      than nextest's process-per-test for this crate; see `AGENTS.md`.


- [x] Restore bench suite (core, infer, lexer, parser, pipeline, typecheck) —
      all six live in `crates/bench`; `core`/`infer` are re-targets of deleted
      crates (see `crates/bench/README.md`). Fixing the restore surfaced that
      `EVAL_SAMPLE` and `kit-canonical/benches/perf.rs` still used the removed
      `|>` pipe syntax, so both benched the error path; replaced with plain
      application and validated with `cargo bench -- --test` (11/11 pass).
      `perf.rs` was *also* never wired up — `crates/kit-canonical/Cargo.toml`
      declared no `[[bench]]`, so libtest's harness swallowed it
      (`running 0 tests`) and criterion never ran; added
      `[[bench]] name = "perf" harness = false` and it now runs (4/4 pass).
- [x] Restore platform/e2e tests (e2e, language_host, integration, mock) —
      `crates/platform-tests` restored for `mock`/`integration`/`e2e` (9 tests,
      green), adapted to the current API: `DefaultHost` replaces the crate's
      `MockHost`, and `MockExtension`/`ExtensionRegistry` are gone from the
      platform. `language_host.rs` is **not** restorable — it targeted the
      removed `LanguageHost`/`SyntaxNode` and the deleted comment plugins; the
      current LSP path is covered by `plugin-lsp`/`plugin-ext-lsp`/`host-lsp`.
      See `crates/platform-tests/README.md`.
- [x] Restore integration tests (fuzz, generics, integration, mock) — **closed
      as unrestorable**: every import targets a deleted crate
      (`handl-ast-canonical`, `handl-store`, `handl-vm*`,
      `handl-typecheck-core`/`-infer`/`-schema-json`, `handl-lib-collections`).
      Coverage is now in the per-crate conformance/parity suites under
      `crates/*/tests/` (`kit-canonical`, `handl-runtime`, `plugin-engine-*`).
      A rewrite would be new work, not a restore.
- [x] Restore ext-tests and compiler-tests after compiler/typecheck refactor —
      `crates/ext-tests` (`e2e.rs`) restored alongside the ADR-0085 capability
      work; the compiler-side end-to-end is `crates/host-cli/tests/`
      (`eval_e2e`, `compile_smoke`, `integration`, `transform`) plus the
      `plugin-engine-exec-*` conformance suites.

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
