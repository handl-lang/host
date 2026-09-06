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
| Linter                 | `plugin-ext-lint`                        | `lint_source(src)`, `check_source(src)`, `fix_source(src)`                                                                                                                                                           |
| Typechecker            | `plugin-engine-typecheck`                | `check_module(src) -> TypecheckReport`                                                                                                                                                                               |
| Test runner            | `plugin-engine-test` (new, ADR-0089)     | `discover(paths)`/`discover_with(&dyn TestProvider)` → `TestPlan`; `filter_predicate`/`tag_predicate`/`tag_expr_predicate` → `Filter` (a handl filter arrow from `handl-packages/pred/pred.hnl` evaluated per case against a `{ name, path, tags }` record projection via `pred.matches`); `run`/`run_with_filter`/`run_selected`/`run_selected_cached`/`run_with_config(&mut RunConfig)` → `RunReport`; `TestEvent` sink; `summarize` → `Summary` → `Reporter` (`HumanReporter`/`JsonReporter`); consumes `plugin-lib-scheduler`, `plugin-exec-cache` (`TestRunCache` via `AstKey`) |
| Test framework library | `handl-packages/test/` (handl contract, ADR-0092) + `plugin-engine-test::framework` (temporary Rust handls) | `assert.*`, `property.for_all`, `benchmark.run`, `fuzz.run`, `snapshot.*`, `test.run` (op interface in `test.hnl`)                                                                  |
| CLI                    | `host-cli`                               | `hnl fmt/lint/check/test/run/eval` subcommands                                                                                                                                                                       |

### CLI exit codes

- `0` — success
- `1` — check/lint/test/fmt-check failure
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

- `handl-defining-handl` cranelift bootstrap test fails on `let` bindings
  (pre-existing, unrelated to ADR 0085-0091 work).
- `plugin-engine-parse-pratt` has an unused import warning (`PayloadTokenSpec`).
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
  which the runner's `TestModuleLookup` resolves `assert` through. No op
  could move to pure handl yet: the language has **no error-raising
  primitive** (assertions must fail loudly) and **no error capture**
  (`assert.throws`/`test.run` need host-side `EvalContext::try_call_value`);
  `property`/`fuzz` additionally need a handl-reachable seeded RNG
  (`rand.seeded_*` lives in `plugin-lib-rand-native` and is wired into the
  canonical module chain) and `benchmark` needs a clock (`time.now_ms`/
  `time.instant_ns` live in `plugin-lib-time-native`, contract in
  `handl-packages/time/time.hnl`, also wired).
  Remaining: add those primitives to the stdlib, then rewrite the
  `handl "..."` bindings as pure-handl `assert.hnl`/`property.hnl`/etc.
  per the ADR target layout.

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
  (the `field_access` native op in `plugin-engine-eval-tree-walk`), not a
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
