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
| Linter                 | `plugin-ext-lint`                        | `lint_source(src)`, `check_source(src)`, `fix_source(src)`                                                                                                                                                           |
| Typechecker            | `plugin-engine-typecheck`                | `check_module(src) -> TypecheckReport`                                                                                                                                                                               |
| Test runner            | `plugin-engine-test` (new, ADR-0089)     | `discover(paths)`/`discover_with(&dyn TestProvider)` → `TestPlan`; `filter_predicate`/`tag_predicate`/`tag_expr_predicate` → `Filter` (a handl filter arrow from `handl-packages/pred/pred.hnl` evaluated per case against a `{ name, path, tags }` record projection via `pred.matches`); `run`/`run_with_filter`/`run_selected`/`run_selected_cached`/`run_with_config(&mut RunConfig)` → `RunReport`; `TestEvent` sink; `summarize` → `Summary` → `Reporter` (`HumanReporter`/`JsonReporter`); consumes `plugin-lib-scheduler`, `plugin-exec-cache` (`TestRunCache` via `AstKey`) |
| Test framework library | `handl-packages/test/` (handl contract, ADR-0092) + `plugin-engine-test::framework` (temporary Rust handls) | `assert.*`, `property.for_all`, `benchmark.run`, `fuzz.run`, `snapshot.*`, `test.run` (op interface in `test.hnl`)                                                                  |
| Transform abstraction  | `plugin-engine-transform` (ADR-0094)     | `TransformEngine` trait, `TransformRequest`/`TransformMode` (`Pass`/`Pipeline`/`ApplyAt`/`Plan`), `TransformOutput`/`RewriteTrace`/`PassStats`, `RuleSource`, `NativeRewrite`/`RewriteCtx`/`NativeRewriteRegistry`, `validate_transforms` |
| Transform engine       | `plugin-engine-transform-rewrite` (ADR-0094) | `RewriteEngine` — `run_pass`/`run_pipeline`/`apply_rule`/`find_matches`, `Traversal` (`BottomUp`/`TopDown`/`Fixpoint`), `DESUGAR_NATIVES`/`with_desugar_natives` (the moved `try_rust_desugar` handlers as named `desugar.*` natives) |
| CLI                    | `host-cli`                               | `hnl fmt/lint/typecheck/test/run/eval/transform` subcommands (`transform <pass> <file> [--pipeline a,b] [--plan] [--trace] [--stats] [--emit tree|json|source] [--write]` — ADR-0094)               |

### Desugar = pass `"desugar"` on the transform engine (ADR-0094)

`handl-runtime::apply_desugar` returns `Result<Ast, LangError>` and
delegates to `RewriteEngine::run_pass("desugar")` with
`with_desugar_natives`; `Runtime::parse` propagates transform errors.
Declarative rules live in `lang.transforms`, authored as first-class
`TransformDef`s in `data/transforms.yaml` (the `AstShapeDef` `desugar:`
field is still lowered at merge for third-party backward compat); kinds
with no declarative rule fall back to the
named `desugar.*` native registry. `validate_transforms` runs at `Language`
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
ADR-0092 — the framework's temporary Rust handls live in
`plugin-engine-test::framework`.

### Source splitting

The Pratt parser parses one statement per call. The typechecker, test runner,
and linter all split source into top-level statements at bracket-depth zero
(respecting strings and `--` comments) before parsing each chunk individually.

### Known issues

- `handl-defining-handl`: `let`/`stmt.let` corpus cases
  (`let_var`/`stmt_let`/`nested_let`/`if_with_let`) are `tree_walk_only`
  because the bytecode and cranelift engines do not implement the `Bind`
  scheme — an engine-coverage gap, not a test failure; the compiled-engine
  tests skip them and `self_host_tree_walk_only_cases` covers them on the
  tree-walk engine. `let` is **not** removed from the language: the
  statement form `let x :: v` parses via `grammar.let.prefix` and evals
  (`eval.let` mirrors `assign`); the `let ... in` expression form was
  removed in ADR-0079 (block bindings `do { x = y; ... }` cover it).
- **Engine-genericity debt** (audit; ~70 residual `kind ==`/positional-child
  hits — these are debt to close via `Language` data, not patterns to extend):
  - `plugin-engine-parse-pratt/src/lib.rs` ~L293/337: `match`/`data` special
    parse branches remain. Closing them needs new generic grammar-row fields:
    an optional keyword clause (`return ... with`) between a fixed child and a
    delimited block for `match`, and per-child binding powers plus a
    `payload_extract` step for `data`.
  - `plugin-engine-exec-bytecode` (`vm.rs`/`compile.rs`) +
    `plugin-engine-exec-cranelift/src/lib.rs`: object-key `var`/`string`
    heuristic — needs a `key` role or `key_is_literal` flag in `object`/`entry`
    `AstShapeDef`.
  - `handl-runtime/src/lib.rs` `collect_module_bindings`: **closed**. Now
    data-driven via `Language::binding_target` using `target` role/`name`
    payload.
  - `plugin-engine-transform-rewrite/src/native/desugar.rs` (moved from
    `handl-runtime/src/desugar_pass.rs`, ADR-0094): `type-annot`/`paren`
    checks in `desugar_match_to_recursor`/`parse_type_param` are data-driven
    via `subject`/`annot`/`body` roles. Remaining `kind ==` in
    `node_to_type_string`, `collect_constructors`, and the
    `while`/`if`/`for`/`nullish`/`opt-chain`/`data`/`match` natives
    (formerly the `try_rust_desugar` registry — now named `desugar.*`
    entries behind `DESUGAR_NATIVES`, still Rust, still bootstrap desugar
    code) are documented as out-of-scope for ADR-0081 unless/until
    declarative desugar rules grow fresh-name generation and recursive
    structure building. The dispatch is now data-driven; the Rust impls
    remain the flagged debt.
  - `plugin-engine-typecheck-nbe/src/lib.rs` `bind_pattern_vars`: **closed**.
    Dispatch uses `AstShapeDef::pattern_role` (`leaf`/`sequence`/`entries`/
    `entry`/`rest`) with a role-less fallback for tests.
  - `plugin-ext-lint` (`var_name_at`), `plugin-engine-test` (`var_name`):
    **closed**. Both now use `Language::param_name` (guarded by
    `pattern_role: leaf`/`subject` role metadata).
  - `host-lsp/src/*` (~20): **closed**. `TokenKindDef` gained optional
    `category`/`pair`/`doc` fields (serde-compatible, free-form strings —
    third-party kits unaffected); `handl-kit-data-canonical/data/tokens.yaml`
    populates them for every canonical kind, `Language` exposes
    `token_category`/`token_in_category`/`token_pair`/`token_doc`/
    `token_kind_for_label`, and `handl-kit-canonical::TokenClasses` is the
    shared classifier. host-LSP, `host-tooling` (semantic tokens +
    TextMate grammar generation), `plugin-ext-fmt` and `plugin-ext-lint`
    now classify via `lang.tokens` — no `token.*` kind strings or
    `is_*_kind` helpers remain outside `kit-canonical`'s legacy shims
    (kept for bootstrap paths and third-party kits that declare no
    categories). Residual
    *identity-specific* checks (not category-answerable, so intentionally
    kept): keyword text matches (`rec`/`law`/`match`/`do`/`return`/
    `axiom`/`color`/snippet+completion tables) — these name a *specific*
    keyword, not a token class; punctuation spellings resolve through
    `is_label`/`token_kind_for_label` rather than kind strings.
  - `plugin-ext-refinement/src/arrow_parser.rs`: arrow-syntax parser —
    treat as bootstrap or move arrow-grammar into `Language` data.
  - `data/eval.yaml`/`data/types.yaml` `let` parity: **closed**. `ast.let`
    (`target`/`value` roles), `grammar.let.prefix` (`let x :: v`),
    `eval.let` (native `assign` op), and `type.let` (synth `type.unit`)
    all exist; `parity_block_*` (46 tests in
    `plugin-engine-typecheck-nbe/tests/parity.rs`) all pass and
    `full_language_parity` no longer exists.
- **ADR-0089 prerequisite (in progress):** (a) `plugin-engine-test` exists as
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
  `TestOutcome` carries `duration` + `FailureDetail`. **Remaining:** wiring
  `hnl test` in `host-cli`, suite grouping (`SuiteStarted`/`SuiteFinished`
  unemitted), structured `expected`/`actual`/`diff` once assert errors carry
  them. ADR-0074 grammar is **not** amended.
- **ADR-0092 (crate deletion done; pure-handl migration blocked):**
  `plugin-lib-test` is **deleted**. The framework contract stays in
  `handl-packages/test/test.hnl` (`op`/`handl` declarations); all 12 Rust
  handls moved into `plugin-engine-test::framework` (`src/framework/`),
  which the runner's `TestModuleLookup` resolves `assert` through. The
  **error-raising** half of the blocker is now closed: `error.raise(v)`
  is a native op on the `error` module (`handl-packages/error/error.hnl`
  contract; `plugin-lib-stdlib::ops::error` handl; registered in the
  canonical module chain). It is callable from any `.hnl` source as
  `error.raise(msg)` — the unbound `error` var falls back to the module
  path like `type.of`/`tag.tag` — or inside table modules via
  `deps: &["error"]`. Payload: a `String` becomes the message verbatim,
  any other value is JSON-serialized into it; it surfaces as
  `EvalError::Other` → `LangError` through `kit.eval`/`hnl eval`.
  `vec`/`list`/`stack`/`queue`/`deque` `pop`/`peek`/`dequeue`/`pop_*`/
  `peek_*` are now pure handl over it (only `new`/`from`/`to_array` stay
  native). The **error-capture** primitive is now added: `error.try(f)`
  calls a thunk and returns a `result`-shaped tagged value
  (`{"__tag__":"ok","value":v}` / `{"__tag__":"err","value":msg}`),
  making `assert.throws`/`test.run` expressible in handl.
  `property`/`fuzz` additionally need a handl-reachable seeded RNG
  (`rand.seeded_*` lives in `plugin-lib-rand-native` and is wired into the
  canonical module chain) and `benchmark` needs a clock (`time.now_ms`/
  `time.instant_ns` live in `plugin-lib-time-native`, contract in
  `handl-packages/time/time.hnl`, also wired).
  Remaining: rewrite the `handl "..."` bindings as pure-handl
  `assert.hnl`/`property.hnl`/etc. per the ADR target layout.

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

## Stashes

- `stash@{0}`: "pre-ADR: result module refactoring" — a `plugin-lib-result`
  crate and related changes from a prior session. Since superseded: the
  `plugin-lib-{hello,result,arrow-iter}` crates were folded into the
  `plugin-lib-stdlib` package table (`MODULES`/`PACKAGES` rows) and
  deleted; `handl-packages/{hello,result,arrow_iter}/` remain the source
  of truth.
