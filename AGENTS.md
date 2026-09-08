# AGENTS.md — Peregrin Platform

## Build & Test

```bash
cargo check                          # workspace check
cargo test --workspace               # full test suite
cargo test -p <crate-name>           # single crate
cargo build -p handl-host-cli        # build the hnl binary
```

## Bootstrap Tooling (ADRs 0085-0091)

The canonical toolchain is implemented in Rust on the generic plugin substrate.
All tools share diagnostics from `plugin-ext-diag` and are dispatched by the
`hnl` CLI in `host-cli`.

### Crate map

| Concern                | Crate                                    | Key API                                                                                                                                                                                                              |
| ---------------------- | ---------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Shared diagnostics     | `plugin-ext-diag`                        | `Diagnostic`, `Severity`, `Span`, `Fix`, `render_diagnostic`, `apply_fixes`                                                                                                                                          |
| Formatter              | `plugin-ext-fmt`                         | `format_source(src)`, `check_source(src)`                                                                                                                                                                            |
| AST formatter          | `plugin-engine-fmt`                      | `FormatEngine::format(lang, ast, root)`, `PreserveEmitter::new(src, ast, root, &changed)?.format(lang, ast, root)`                                                                                                   |
| Linter                 | `plugin-ext-lint` (wiring) + `plugin-engine-lint` (engine, ADR-0097) | `lint_source(src)`, `check_source(src)`, `fix_source(src)`; rules are `LintRuleDef`s in `lang.lints` (`data/lints.yaml`): `!Match { node_kind, when }` uses the `TransformCondition` vocabulary; `!Native "name"` resolves via `NativeLintRegistry` (`canonical_lint_natives` = `scope.*`/`naming-convention`/`source.missing-module-doc`; `plugin-ext-lint` registers `source.fmt`)                                                                                                            |
| Typechecker            | `plugin-engine-typecheck`                | `check_module(src) -> Result<TypecheckReport, CheckError>`                                                                                                                                                           |
| Test runner            | `plugin-engine-test` (new, ADR-0089)     | `discover(paths)`/`discover_with(&dyn TestProvider)` → `TestPlan`; `filter_predicate`/`tag_predicate`/`tag_expr_predicate` → `Filter` (a handl filter arrow from `handl-packages/pred/pred.hnl` evaluated per case against a `{ name, path, tags }` record projection via `pred.matches`); `run`/`run_with_filter`/`run_selected`/`run_selected_cached`/`run_with_config(&mut RunConfig)` → `RunReport`; `TestEvent` sink; `summarize` → `Summary` → `Reporter` (`HumanReporter`/`JsonReporter`); consumes `plugin-lib-scheduler`, `plugin-exec-cache` (`TestRunCache` via `AstKey`) |
| Test framework library | `handl-packages/test/` (pure-handl, ADR-0092) + `plugin-engine-test::framework` (assert side-channel only) | `assert.*`, `property.for_all`, `benchmark.run`, `fuzz.run`, `snapshot.*`, `test.run` (op interface in `test.hnl`)                                                                  |
| Transform abstraction  | `plugin-engine-transform` (ADR-0094)     | `TransformEngine` trait, `TransformRequest`/`TransformMode` (`Pass`/`Pipeline`/`ApplyAt`/`Plan`), `TransformOutput`/`RewriteTrace`/`PassStats`, `RuleSource`, `NativeRewrite`/`RewriteCtx`/`NativeRewriteRegistry`, `validate_transforms` |
| Transform engine       | `plugin-engine-transform-rewrite` (ADR-0094) | `RewriteEngine` — `run_pass`/`run_pipeline`/`apply_rule`/`find_matches`, `Traversal` (`BottomUp`/`TopDown`/`Fixpoint`), `DESUGAR_NATIVES`/`with_desugar_natives` (the remaining semantic `desugar.data`/`desugar.match_recursor` natives) |
| CLI                    | `host-cli`                               | `hnl fmt/lint/typecheck/test/run/eval/transform` subcommands (`transform <pass> <file> [--pipeline a,b] [--plan] [--trace] [--stats] [--emit tree|json|source] [--write]` — ADR-0094)               |

### Desugar = pass `"desugar"` on the transform engine (ADR-0094)

`handl-runtime::apply_desugar` returns `Result<Ast, LangError>` and
delegates to `RewriteEngine::run_pass("desugar")` with
`with_desugar_natives`; `Runtime::parse` propagates transform errors.
Declarative rules live in `lang.transforms`, authored as first-class
`TransformDef`s in `data/transforms.yaml` (the `AstShapeDef` `desugar:`
field is still lowered at merge for third-party backward compat) — that
includes the five fixed-shape sugar forms (`while`, `do-while`, `for`,
`nullish`, `opt-chain`), which migrated out of Rust once `RewriteStep::
Fresh` landed (their `__continue`/`__result`/`rec_v`/`rec_tail` binders
are minted per-application as `__fresh_<hint>_<n>` names) — no
`Apply`-style recursion is needed either (bottom-up `Keep`/`KeepRest`
already supplies rewritten subterms). Only `data` and `match` still fall back to the
named `desugar.*` native registry (`desugar.data`,
`desugar.match_recursor`): they parse type expressions out of payloads,
iterate a variable number of constructors, run strict-positivity
checking, and (for `match`) search the arena for `data` declarations —
genuinely semantic work that would require per-constructor iteration /
environment lookup, i.e. a macro system, not a rewrite step.
`validate_transforms` runs at `Language`
build in `handl-kit-data-canonical::merge_fragments` — Error-severity
diagnostics fail assembly — and under `hnl typecheck`, whose bootstrap
`Language` assembly in `plugin-engine-typecheck::bootstrap` mirrors
`merge_fragments` (embedded `transforms.yaml` + `merge_ast_fragment`
lowering + validation; errors surface as `CheckError::Language`).

`.hnl`-authored rewrites are supported via the `Native("handl:<mod>.<fn>")`
spelling: `handl_plugin_lib_handl::transform::HandlNativeRewrite` is a
`NativeRewrite` that resolves `handl:` names against registered
`HandlModule`s and invokes the function with a `{kind, span, payload,
children}` record; the function returns `null` (decline) or a node spec
`{kind, payload?, children?}` where children may be `{"__keep__": n}`
markers reusing the matched node's remapped child `n`. See ADR-0094 §"Handl-
authored transforms" and `plugin-lib-handl/tests/handl_transform.rs`.

### CLI exit codes

- `0` — success
- `1` — typecheck/lint/test/fmt-check failure
- `2` — usage or parse error

### Engine genericity (ADR-0081)

All tools resolve AST child positions via `Language::child_role_index(kind,
role)` — never hardcoded `kind ==` checks. Fall back to positional only for
shapes that declare no roles.

### Cyclic dependency workaround

`plugin-engine-typecheck` and `plugin-ext-lint` embed YAML language data
directly via `include_str!` instead of depending on `handl-kit-canonical`,
because `handl-kit-canonical` → `handl-runtime` → `plugin-engine-typecheck`
would create a cycle. `plugin-ext-fmt` and `plugin-engine-test` (ADR-0089)
use `handl-kit-data-canonical` (no eval engine) or
`handl-kit-canonical` respectively. `plugin-lib-test` is deleted per
ADR-0092; the framework is now pure-handl under `handl-packages/test/`
(registered by `handl-kit-canonical`), and `plugin-engine-test::framework`
retains only the `assert` failure side-channel.

### Source splitting

The Pratt parser parses one statement per call. The typechecker, test runner,
and linter (`plugin-engine-lint::parse_module`, ADR-0097) all split source
into top-level statements at bracket-depth zero (respecting strings and `--`
comments) before parsing each chunk individually.

### Dogfood gate status

The `hnl fmt --check && hnl lint && hnl typecheck && hnl test` gate over
`handl-packages/` and `examples/` is scripted in `scripts/dogfood-gate.sh`
and wired in `.github/workflows/dogfood-gate.yml` as an informational CI job
(`continue-on-error: true`).

Measured numbers from this session:

- parse failures: 0 (down from 30)
- typecheck failures: ~57, all `[unbound-wire]` (binder/match-arm scoping and module-env gaps)
- lint failures: ~135

The gate is not yet green, so it stays informational. Remove
`continue-on-error: true` once `hnl fmt --check`, `hnl lint`, `hnl typecheck`,
and `hnl test` are clean over the dogfood corpus.

### Unified import model (ADR-0096)

- **Canonical surface forms** (all → `stmt.import`):
  `import "<spec>"`, `from "<spec>" import *`, `from "<spec>" import * as
  <alias>`, `from "<spec>" import <name> [as <alias>]`. The `from` tail is
  `AstShapeRef::trailing_tokens` on `grammar.stmt-from.prefix`: `import`,
  then `*` or an identifier captured into `payload.names`, then an optional
  `as <alias>` gated by `TokenStep::requires_prev_match` (new flag — a
  gated step is only attempted when the previous step matched, so the alias
  never swallows a following statement's leading identifier).
- **Path vs module-name specifiers**: a `<spec>` resolving to a file
  (`.handl.toml` `[dependencies]` first segment → package dir; else module
  dir → module root → CWD) is the executable import op — `prim_import` in
  all three eval engines splices exports, binds the module record under
  `* as`, or projects a named export (missing name → `module has no
  export`). A spec that does **not** resolve to a file (bare host-module
  names like `str`, extension-less siblings like `./dedent`) denotes a
  module on the host chain: `HandlModule::with_resolver` auto-wires it for
  `.hnl` module sources, and whole-program/file-loader hosts strip the
  declaration line (`host-cli::strip_import_declarations`,
  `ModuleLoader::strip_module_decl_lines`, the test runner's `push_chunk`)
  because the kit's module chain already supplies the bindings. A
  `resolve_import` module-name fallback (registry → module-namespace
  value) is deferred pending `loader.hnl` default install.
- **`hnl typecheck`** scopes every `stmt.import`'s bound names (child var,
  `payload.names`, `payload.alias`) at module level and inside `do { ... }`
  blocks — statements bind sequentially, so block-level imports and
  `assign` targets are in scope for following siblings.
  `from "<bare-name>" import *` scopes the module name itself; path splice
  imports bind statically unknown names (still flagged — use `* as` or the
  named form).
- **Arrow-module dialect** (`plugin-ext-refinement`): `import x from "p"`,
  `import a, b from "p"`, `export ... from` are the arrow-module line
  scanner's syntax — dialect-fixed, not canonical. Canonical Pratt parses
  `import x from "p"` as `import (x)` and silently drops the `from "p"`
  tail (no cursor-exhaustion check); `hnl run`/the test runner strip the
  line.

### Known issues

- `let` is **fully removed** from the language (completes ADR-0079):
  `token.let`, `lex.let`, `grammar.let.prefix`, `ast.let`, `eval.let`, and
  `type.let` are deleted. `token.keyword.let` + `lex.keyword.let` stay as a
  **tombstone** (same pattern as `then`/`axiom`): `let` remains a reserved
  word with no grammar row, so `let x :: v` fails with "unexpected token"
  instead of silently lexing as an identifier. Bindings are plain
  `x = y` / `x :: y` (both lex to `token.bind` → `assign` nodes);
  `do { x = y; ... }` blocks cover the old `let ... in` form. The internal
  `stmt.let` kind is also gone — nothing produced it (no grammar row, no
  desugar native, no transform); block bindings eval directly as `assign`
  children of `block` via `eval_block`. `x = y` in a block is
  **update-or-define** (`ScopeStack::assign` in all three engines —
  tree-walk, bytecode, cranelift): it updates the nearest enclosing
  binding when `x` is already bound, else defines a block-local binding —
  that is what lets `while`/`do-while`/`for` bodies rebind outer
  variables (`x = x + 1`) and lets the desugars' `__continue`/`__xs`
  feedback wires reach the `rec` region's bindings. `rec` fixpoint
  convergence observes those cross-scope writes via
  `ScopeStack::enclosing_updated` plus an end-of-pass comparison of each
  rec binding's env value (nested writes are invisible to assign-time
  change detection); the `eval_rec` native op is implemented in all
  three engines (`prim_eval_rec` / `prim_v2_eval_rec`), so the
  `rec`+`assign` structures the `desugar` pass produces for
  `while`/`do-while`/`for` run identically on the compiled engines.
  The `handl-defining-handl` corpus cases that modelled `let`/`stmt.let`
  were rewritten to `block`+`assign` form
  (`bind_var`/`bind_stmt`/`nested_bind`/`if_with_bind`/`proc_bind_body`,
  plus `rest_params`/`rest_params_empty` — **no corpus case is
  `tree_walk_only` any more**; `function_call`/`pipe_call`/`if_true`
  were stale flags). `tests/bind_rec_parity.rs` gates the semantics
  end-to-end: real `do {}` rebinding and `while`/`do-while`/`for`
  sources are parsed (desugar runs at parse time) and run through
  tree-walk, bytecode, and cranelift with identical results. Note:
  several engine/test crates still use a kind literally
  named `"let"` inside **self-contained toy `Language`s** (e.g.
  `plugin-engine-exec-*` unit tests, `handl-test-utils` conformance
  corpus, `plugin-engine-lint`/`plugin-engine-fmt` fixtures,
  `handl-runtime/tests/runtime_eval_only.rs`) — those kinds are local to
  their test languages and unrelated to canonical surface syntax.
- **Engine-genericity residual audit (final)**. A fresh sweep of `crates/`
  for `kind == "`, `== "token.`, `starts_with("token.`, and `.kind.*==`
  (excluding test fixtures, toy `Language`s, and the documented bootstrap
  exceptions) leaves **~30 residual hits**. The previous ~70 count was stale;
  the major role/token/grammar work is closed. The remainder is deliberate
  bootstrap/identity-specific/role-less-fallback code. Per-item rationale:
  - **(a) Closed in this pass:**
    - `plugin-engine-typecheck-nbe/src/predicate_eval.rs` `function_param_body`:
      the bare `param_node.kind == "var"` check is now `Language::param_name`,
      which unwraps `type-annot.subject` role data as well as bare `var`.
  - **(b) Intentionally kept (bootstrap or identity-specific):**
    - `plugin-ext-refinement/src/arrow_parser.rs` **= bootstrap**. The arrow
      combinator vocabulary (`arr`, `comp`, `first`, `choice`, `loop`, `app`,
      `id`, `flow`) is calculus-kernel syntax, not surface-language syntax,
      so materializing it into `Scheme` in Rust is allowed per the rule
      "adding a calculus combinator = engine edit". The hand-rolled
      import/export/`op`/`handl`/`cap`/`tag` line scanner is also intentional
      bootstrap: arrow modules use `import x from "path"` (and
      `export ... from`) which is the inverse order of the canonical Pratt
      `stmt-from`/`stmt-import` rows, and the loader must resolve imports
      before arrow expressions are parsed. The
      `from "./x" import { dedent }` brace form was handled by **neither**
      the arrow-loader line scanner nor the canonical `stmt-from` row; the
      single call site (`handl-packages/dedent/dedent.test.hnl`) was
      rewritten to canonical `from "./dedent" import * as dedent`, so the
      divergence is **closed**. Note the import line in `.test.hnl` files is
      still skipped textually by the test runner (and stripped by `hnl
      run` when its specifier does not resolve to a file) — the stdlib
      module chain supplies the bindings — but the line now also parses
      under canonical Pratt, so `hnl typecheck`/`lint`/`fmt` accept the
      file. See "Unified import model (ADR-0096)" below.
    - `plugin-engine-transform-rewrite/src/native/desugar.rs`: only the
      `data` and `match`→recursor desugar natives
      (`node_to_type_string`, `collect_constructors`, strict positivity,
      arena-wide `data`-declaration lookup) stay Rust — genuinely semantic
      rewrites needing constructor-list iteration and environment lookup
      that the `RewriteStep` vocabulary cannot express without growing
      into a macro system. The fixed-shape forms (`while`/`do-while`/
      `for`/`nullish`/`opt-chain`) are declarative `TransformDef`s in
      `transforms.yaml` — `RewriteStep::Fresh` mints their binders, and
      bottom-up `Keep` supplies the recursion. The unregistered
      `desugar_if` (never in the pass; `type.if` must observe `if` nodes)
      was deleted — the identical rewrite is covered by the declarative
      `if`→`match` tests.
    - `plugin-engine-typecheck/src/bootstrap.rs` and related YAML-assembly
      paths: these are the bootstrap `Language` builder (cyclic-dependency
      workaround) and are allowed to name canonical top-level kinds such as
      `var`, `import`, `export`, `rec`, `entry`, `field-access`.
    - `plugin-engine-exec-bytecode` (`vm.rs`/`compile.rs`) +
      `plugin-engine-exec-cranelift/src/lib.rs`: object-entry key literal
      detection is now data-driven for canonical `entry` via
      `Language::literal_child` (`literal_children: { key: [var, string] }`);
      the `var`/`string` fallback only fires for role-less shapes and is an
      acceptable legacy/test-language fallback.
    - `plugin-engine-exec/src/pattern_match.rs`,
      `plugin-engine-exec-tree-walk/src/lib.rs`,
      `plugin-engine-typecheck-nbe/src/lib.rs` `bind_pattern_vars_legacy`:
      canonical pattern dispatch is already `AstShapeDef::pattern_role`-driven;
      residual `kind == "var"/"array"/"object"/"call"/"rest"/"entry"` are
      role-less fallbacks for test/legacy languages plus `call` tag-pattern
      recognition, which names a specific construct.
    - `plugin-engine-typecheck-nbe/src/totality_check.rs`:
      `node.kind == "assign"` is a role-less binding fallback;
      `node.kind == "call"` in `find_recursive_calls` is identity-specific
      recursion detection.
    - `plugin-engine-typecheck-nbe/src/predicate_eval.rs` `function_param_body`:
      the legacy `children: [param, body]` layout is still guarded by
      `node.kind == "function"`. `function` is a core calculus combinator
      and the role-less `Language::new()` used in NBE unit tests has no
      `AstShapeDef` for it, so the identity guard remains; only the inner
      `param_node.kind == "var"` was closed (replaced by `Language::param_name`).
    - `plugin-exec-map/src/lib.rs`, `plugin-exec-logging/src/lib.rs`: small
      exec-control plugins that extract a `call` callee name from a `var`;
      identity-specific.
    - `host-lsp/src/*` and `crates/kit-canonical/src/lib.rs`: residual
      keyword text matches (`rec`/`law`/`match`/...) and legacy token-class
      shims are identity-specific or for bootstrap/third-party kits.
    - `plugin-engine-parse-pratt/src/lib.rs`: the generic Pratt interpreter
      still inspects token kind strings supplied by grammar data; the handful
      of `token.lbrace`/`token.comma`/`token.arrow.comma` checks are
      structural delimiter detection, not language-specific dispatch.
  - **(c) Genuinely blocked on missing vocabulary:**
    - None of the remaining hits are cheaply closable; the few that could move
      into `Language` data (e.g. a `pattern_role: tag` for `call` patterns,
      or a generic `is_call`/`is_binder` role) would add new vocabulary and
      are out of scope for this cleanup.
  - `data/eval.yaml`/`data/types.yaml` `let` parity: **closed by removal**.
    `let` is gone end-to-end (`token.let`, `lex.let`, `grammar.let.prefix`,
    `ast.let`, `eval.let`, `eval.stmt-let`, `type.let` all deleted;
    `token.keyword.let`/`lex.keyword.let` remain only as a reserved-word
    tombstone); `parity_block_*` (46 tests in
    `plugin-engine-typecheck-nbe/tests/parity.rs`) all pass.
- **ADR-0089 prerequisite (done):** (a) `plugin-engine-test` exists as
  a standalone crate; (b) the **test-language fragment**
  (`plugin-engine-test/data/test-language.yaml`) declares
  `test`/`property`/`integration_test`/`bench` `AstShapeDef`s (with
  `name`/`body` roles — the name child parses as a `var` node; the rows use
  `binding_power [6, 0]` so the name stops before infix `=`, the same trick
  as `data`), `GrammarRow`s, `LexerRule`s, and `EvalSchemeDef`s, merged onto
  the canonical kit at discovery time via `Language::merge` (ADR-0044) — the
  legacy `test_*` binding convention remains as a fallback; (c) generic
  systems live in dedicated crates — `plugin-lib-scheduler` (bounded-job
  `BoundedScheduler`/`SchedulePolicy`) and filtering, which is now a
  **handl arrow**: `plugin-lib-predicate` is deleted and its combinator
  vocabulary (`and`/`or`/`not`/`field`/`exact`/`glob`/`always`/`never`/
  `match_glob`) lives in `handl-packages/pred/pred.hnl`, evaluated per
  case against a `{ name, path, tags }` record projection — plus
  `handl-language::capabilities` and `plugin-exec-cache` (`TestRunCache` keys
  results by `AstKey{kind:"test.result", module+name+body_source}`). The
  runner now consumes all of them: `run_with_config(&mut RunConfig)` is the
  single `--filter`/`--jobs`/`--cache` entry point, `TestEvent` covers the
  ADR event vocabulary (Vec sink — no event bus until a second consumer),
  `TestProvider`/`Reporter` are the provider and reporter seams, and
  `TestOutcome` carries `duration` + `FailureDetail`. `hnl test` is wired
  in `host-cli` (`--filter`/`--tag`/`--jobs`/`--cache`/`--report
  human|json`/`--list`/`--changed`). The full event vocabulary is emitted:
  `SuiteStarted`/`SuiteFinished` bracket each **module** (the suite
  boundary — `TestPlan` has no `Suite` concept; a suite is a contiguous
  run of one module's cases in canonical plan order). Structured
  `expected`/`actual`/`diff` flow from both failure paths into
  `FailureDetail`: the engine `assert` keyword via the
  `TestAssertHandler` thread-local side-channel, and the pure-handl
  `assert.*` helpers (`handl-packages/test/assert.hnl`) via their `err`
  payload (`failure_detail_from_value`); both reporters render them
  (human: `== <module> ==` suite headers + indented detail lines; json:
  `suites` array + per-test fields). `diff` is only produced by the
  engine `assert` path — the `.hnl` helpers don't emit one yet.
  ADR-0074 grammar is **not** amended.
- **ADR-0092 (pure-handl migration done):**
  `plugin-lib-test` is **deleted**. The framework now lives as pure handl
  in `handl-packages/test/` (`assert.hnl`, `property.hnl`, `benchmark.hnl`,
  `fuzz.hnl`, `snapshot.hnl`, `test.hnl`). `handl-kit-canonical` builds and
  registers these modules through the generic `HandlModule` seam; their
  `op`/`handl` declarations are stripped at load time. The
  `plugin-engine-test::framework` crate no longer contains the temporary
  Rust handls — it only keeps the thread-local assert-failure side-channel
  consumed by the runner's `TestAssertHandler`. The framework uses the
  **result pattern** only (`{"__tag__":"ok",...}` / `{"__tag__":"err",...}`);
  it neither raises nor catches errors directly. The engine continues to
  convert per-case `EvalError` into a failed `TestOutcome` in the runner,
  and `assert.*` helpers build `err` result values directly. The
  `rand.seeded_*` RNG and `time.instant_ns` clock used by the framework
  were already wired into the canonical module chain; `error.raise` and
  `error.try` are not used by the test framework.

## Library packaging (ADR-0077 fold-in)

Handl-authored libraries are **data, not crates**: `handl-packages/*` holds
the contracts (`.hnl` `cap`/`op`/`handl` declarations) and pure-handl impls;
`crates/plugin-lib-stdlib` holds the package table (`src/packages.rs` —
`MODULES`: name → `.hnl` + funcs + ops + deps; `PACKAGES`: library name →
modules) plus the native op impls (`src/ops/`); `plugin-lib-*-native` crates
are FFI atom backends; `plugin-lib-handl` is the loader seam
(`HandlModule`/`HandlLibraryPlugin`/`ModuleResolver`). `plugin-ext-kit`'s
`populate` registers table libraries via `stdlib::library(name)`; modules
with deps on non-table natives (e.g. `result` → `tag`) are built via
`module_with`/`with_resolver_fallback`.

### Loader self-hosting (ADR-0094) — status: canonical in handl, bootstrap on the hot path

`handl-packages/loader/loader.hnl` is the **complete canonical
implementation** of the `.hnl` text transforms: `strip_comments`,
`strip_decls`, `transform_export_to_object` (incl. `find_binding_sep`),
`parse_imports`, `parse_caps`, `parse_tags`, `expand_tags`,
`escape_string`, plus the composites `prepare` and `analyze`. In
`plugin-lib-handl`:

- `mod bootstrap` holds the irreducible Rust copies — needed to prepare
  `str-handl` itself (the bootstrap paradox: the loader needs `str`,
  `str` needs the transforms) and as the correctness fallback.
- `HandlLoader` + `install_loader`/`loader()` are the opt-in self-hosting
  seam: once installed (e.g. `HandlLoader::new(vec![char_mod, str_mod])`),
  `HandlModule::new`/`analyze_source` and every public transform delegate
  to `loader.hnl`, falling back to `bootstrap` on loader error.
- `plugin-lib-handl/tests/loader.rs` is a parity test (handl vs bootstrap,
  small corpus) — **all transforms agree**.

**Why the handl loader is not the default yet:** `HandlFunction::call`
now caches the parsed applied-form `Ast` per (module, function, arity) in
a thread-local `AST_CACHE` and evaluates it via
`Runtime::eval_shared_with_globals` / `EvalEngine::eval_shared` (an
`Rc<Ast>` seam that eliminates both per-call arena clones on the
tree-walk engine), so parsing is no longer the bottleneck. Measured on
`examples/loader_bench.rs` (release): a warm `str.length` call went
~6.5ms → ~0.33ms (~19×); `loader.strip_decls` on `dedent.hnl` (2.7k
chars) ~6.7s → ~0.93s (~7×); but on `str-handl.hnl` (22k chars) ~91s →
~43s. The residual cost is the *interpreted* `str.*`/`char.*` body eval
over every input character (record caching is impossible across the
`serde_json::Value` boundary — arrows' captured `env`s expand
exponentially for `do {}`-block modules like `str-handl`). ~43s for one
22k-char transform — minutes across a kit's ~30 modules — is still far
too slow, so `kit-canonical` deliberately does **not** `install_loader`
(see the NOTE in `build_kit`). Promotion to the default path awaits a
compiled eval engine (bytecode/cranelift) for the transform pass, or a
`TaggedValue`-level cache that never serializes captured envs. Until then
the Rust `bootstrap` seam is the minimal *active* implementation and
`loader.hnl` is the verified spec.

## String / Char / Unicode layering (ADR-0093)

The `str`/`char`/`unicode` surface is layered: a **backend-neutral contract**
(`.hnl` axioms + laws) is separate from **implementations** (pure-Handl and
runtime-specific backends), mirroring the `encoding`/`url`/`ws` pattern.

```text
handl-packages/str/str.hnl       contract: 45 axioms + laws (no impl)
handl-packages/char/char.hnl     contract: 18 char axioms (semantic names —
                                 `is_whitespace`/`is_alpha`, NOT `is_ascii_*`)
handl-packages/str-handl/str-handl.hnl   canonical `str` impl: pure handl
                                         over the `char` floor (char.chars /
                                         char.from_chars for O(n) iter)
handl-packages/char-native/      `char` backend: Rust `std::char` (Unicode
                                 scalar values, not bytes)
handl-packages/unicode/unicode.hnl  contract: graphemes/normalization/
                                    casefold/width (UAX #29/#15/#11)
handl-packages/unicode-handl/       `unicode` impl: code-point-level subset
                                    over `char.*` (graphemes/width are
                                    code-point fallbacks; `nfc`/`nfd` are
                                    backend-required and omitted from the
                                    record so a composite backend can fail
                                    loudly if no native layer is wired)
crates/plugin-lib-char-native    `CharModule` — registers `char.*` ops
crates/plugin-lib-unicode-native `UnicodeModule` — ICU4X-backed native ops
                                 (graphemes/width/casefold/nfc/nfd/decode/
                                 identifiers) composed with `unicode-handl`
```

- `kit-canonical` registers `str-handl` under `"str"` **first** (before the
  all-FFI `str` table row in `plugin-lib-stdlib` — `ops::strs`, the former
  `plugin-lib-utils` `StrModule`), so `str.*` and `from "str" import`
  resolve to the handl impl. `char` resolves via `StdlibResolver` →
  `CharModule`.
- `kit-canonical` registers `handl-plugin-lib-unicode-native::UnicodeModule`
  under `"unicode"`; native `op`s take precedence and the `unicode-handl`
  record supplies fallback behavior for anything not implemented natively.
- `str-handl` must depend only on `char.*` atoms — never on Rust/Js/UTF-8 or
  byte offsets. `char.chars`/`char.from_chars` exist precisely so O(n) bulk
  conversion replaces O(n²) `char.at` loops.
- **Runtime fix (field-access):** `eval.field-access` is `source: Native`
  (the `field_access` native op in `plugin-engine-exec-tree-walk`), not a
  `!Foreign` host call. Passing a record of closures through the
  `serde_json` boundary serialized each `Arrow`'s captured env recursively —
  exponential in the number of prior bindings (OOM at ~20 helpers). Native
  `field_access` keeps `TaggedValue::Obj`/`Arr` in-engine.
- **Runtime fix (nested eval):** `TreeWalkEngine::eval` saves/restores the
  `CURRENT_AST`/`CURRENT_LANG`/`CURRENT_MODULES`/`CURRENT_GLOBALS`
  thread-locals instead of clearing them, so a `.hnl` module function
  calling another `HandlFunction` (e.g. `dedent` → `str.*` → `str-handl` →
  `char.*`) doesn't clobber the outer runtime's state — `call_value` /
  `TreeWalkCallable` depend on them.
- **Reentrancy fix:** `HandlFunction` caches per-thread `Rc<Runtime>` and
  clones the `Rc` out of the `RefCell` before `run` so nested module calls
  don't hit `RefCell already borrowed`.
- **Runtime fix (arrow-value boundary):** `HandlFunction::call` binds
  arguments as per-call globals (`__arg_N`, via
  `Runtime::eval_with_globals`) instead of serializing them into the
  applied source text — closures, tagged values, and other reserved-key
  values cross the boundary as *values*. Arrows that cross carry a
  `"__home__"` frame id (registered in the tree-walk home-frame registry —
  `current_home_frame`/`register_home_frame`) so `apply_app`/`call_value`
  route the `body` index back to the AST + Language + module chain that
  minted it. `apply_app` also curries surplus args (`x ~> y ~> body`
  applied to two args applies `x` then applies the result to `y`).
- The former `plugin-lib-utils` `StrModule` + `handl-packages/utils/str.hnl`
  are **demoted to a backend**: `plugin-lib-utils` is folded into
  `plugin-lib-stdlib` — the `str` MODULES row dispatches `str.*` ops via
  `ops::strs` — and `kit-cli`/`kit-stdlib`/`kit-server`/`host-lsp` consume
  `str.*` through `plugin-ext-kit::populate` → `library("utils")` without
  wiring the `char` resolver, so the FFI ops stay the non-canonical
  fallback. The canonical `"str"` is `str-handl`.
- **`hnl typecheck` dialect routing (dogfood-sweep reconciliation).** Not
  every `.hnl` file is canonical surface syntax: `handl-packages/` mixes
  three dialects — (a) canonical handl, (b) **contract files**
  (`axiom <name> : T -> T` + indented `requires:`/`guarantee:`/
  `termination:` clauses; `contracts/*.hnl`, `str`, `char`, `time`,
  `unicode`), and (c) **arrow-module files** (top-level `cap`/`op`/`handl`/
  `tag` declarations and arrow-graph expressions — `arr(`/`first(`/
  `choice(`/`loop(`/`comp(`/`app(`/`|||`/`rec { ... }` simple form — e.g.
  `state/*`, `fn/*`, `collection/*`, `parse-*`). `typecheck_one` in
  `host-cli` now routes: contract files (first top-level line starts with
  `axiom `) go to `plugin_ext_refinement::parse_contracts`; canonical
  parse failures or canonical-files-with-arrow-markers-that-error are
  retried through `parse_arrow_module` (which itself runs the canonical
  grammar data-driven, then a legacy arrow fallback). Routing lives in
  `host-cli` because `plugin-engine-typecheck` → `plugin-ext-refinement`
  would cycle (`refinement` → `kit-data-canonical` → `handl-runtime` →
  `typecheck`). Sweep result: **30 → 0 parse failures** across the 177-file
  corpus; typecheck failures 128 → 57, all remaining `[unbound-wire]`
  (binder/match-arm scoping and module-env gaps — a separate class).
  - `axiom` is no longer a pure tombstone in the canonical language:
    `grammar.axiom.prefix` parses `axiom name body` to a `law` node (the
    documented alias). The contract-file `axiom name : sig` dialect still
    belongs to `parse_contracts`, which now also accepts the Unicode
    spellings `→` (arrow), `×` (product), `∧`/`∨` (and/or) used by
    `handl-packages/contracts/*.hnl`. Note `handl-packages/contracts/` and
    the crate-bundled `plugin-ext-refinement/contracts/` have **diverged
    semantically** (the bundled copies are older ASCII variants — e.g.
    `axiom cons` vs `axiom list.cons`); the loader accepts both spellings
    but the contents differ — reconciling them is a contract-content
    decision left open.
  - `->` and `|>` never lexed in any dialect — dead syntax. The five
    `cli-framework` files that used the speculative `f :: (p: T): R ->`
    signature / `x |> (p) -> body` pipe dialect were rewritten to canonical
    `f :: (p: T) ~> body` + direct calls; `examples/handl/composition.hnl`
    was rejoined to a single `5 >>> ...` statement (the statement splitter
    breaks at depth-0 newlines).
  - `:=` appears only inside `--` comments — no token needed.
  - Call-site spread `f(...xs)` is **deferred**: no live source uses it
    (only doc comments); `...` (token.ellipsis) already parses in
    array/object/rest positions. `...` and `>>>` are distinct tokens — the
    claimed prefix ambiguity does not exist. Real support needs an
    eval-scheme splice (a `proj_rest`/apply-path op that expands `rest`
    children into the arg list), not just a `rest_token` row on `call` —
    a language-design decision, not a sweep fix.
  - The arrow parser's named-arrow path now actually splits ` :: ` (it was
    detected by `has_named_arrows` but never split — `name :: rec {...}`
    files failed with "no named arrows found") and routes bodies through
    `parse_arrow_expr` so `rec { ... }` bodies work in named arrows.
  - Match-arm `{ stmt; stmt }` multi-statement bodies (`fn/*`,
    `collection/*` rich-rec files) were left in arrow-dialect form — they
    parse via `parse_arrow_module`'s legacy path and are consumed by the
    refinement pipeline today (`state_tests` asserts their `Scheme`s).
    Canonical multi-statement bodies remain `do { ... }`.

## Stashes

- `stash@{0}`: "pre-ADR: result module refactoring" — a `plugin-lib-result`
  crate and related changes from a prior session. Since superseded: the
  `plugin-lib-{hello,result,arrow-iter}` crates were folded into the
  `plugin-lib-stdlib` package table (`MODULES`/`PACKAGES` rows) and
  deleted; `handl-packages/{hello,result,arrow_iter}/` remain the source
  of truth.
