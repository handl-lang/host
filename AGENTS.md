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
| Test runner            | `plugin-engine-test` (new, ADR-0089)     | `discover(paths) -> TestIndex`, `filter(index, predicate) -> TestPlan`, `plan(plan, policy) -> ExecutionPlan`, `run(plan) -> RunReport`, `run_with_filter(plan, filter)`, `format_report_human`/`format_report_json` |
| Test framework library | `handl-packages/test/` (handl, ADR-0092) | `assert.*`, `property.for_all`, `benchmark.run`, `fuzz.run`, `snapshot.*`, `test.run` (op interface in `test.hnl`)                                                                                                   |
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
would create a cycle. `plugin-ext-fmt` and `plugin-engine-test` (new,
ADR-0089) use `handl-kit-data-canonical` (no eval engine) or
`handl-kit-canonical` respectively. (`plugin-lib-test` is being deleted per
ADR-0092.)

### Source splitting

The Pratt parser parses one statement per call. The typechecker, test runner,
and linter all split source into top-level statements at bracket-depth zero
(respecting strings and `--` comments) before parsing each chunk individually.

### Known issues

- `handl-defining-handl` cranelift bootstrap test fails on `let` bindings
  (pre-existing, unrelated to ADR 0085-0091 work).
- `plugin-engine-parse-pratt` has an unused import warning (`PayloadTokenSpec`).
- **ADR-0089 prerequisite (not yet done):** the production test runner
  requires (a) a new `plugin-engine-test` crate extracted from
  `plugin-lib-test`, (b) a **test-language fragment** — a YAML
  `Language` fragment declaring `test`/`property`/`integration_test`/`bench`
  `AstShapeDef`s (with `name`/`body` roles), `GrammarRow`s,
  `LexerPattern`s, and `EvalSchemeDef`s — merged onto the canonical kit at
  discovery time via `Language::merge` (ADR-0044), and (c) reusing generic
  platform systems rather than reinventing them: capability reasoning from
  `handl-language::capabilities`, eval-result memoisation from
  `plugin-exec-cache`, and a bounded-job scheduler + predicate combinators
  consumed as generic systems (extracted to dedicated crates when a second
  consumer appears; until then the runner may ship simple in-crate
  implementations per ADR-0085 rule 6). ADR-0074 grammar is **not** amended.
- **ADR-0092 (proposed):** `plugin-lib-test` is deleted once the test
  framework library is migrated to handl under `handl-packages/test/` and
  the runner has been extracted to `plugin-engine-test` (ADR-0089). Until
  then `plugin-lib-test` still hosts both concerns.

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
                                    backend-required and omitted so they
                                    fail loudly rather than return
                                    un-normalized text)
crates/plugin-lib-char-native    `CharModule` — registers `char.*` ops
```

- `kit-canonical` registers `str-handl` under `"str"` **first** (before any
  legacy `StrModule` fallback in `plugin-lib-utils`), so `str.*` and
  `from "str" import` resolve to the handl impl. `char` resolves via
  `StdlibResolver` → `CharModule`.
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
- `plugin-lib-utils`'s `StrModule` + `handl-packages/utils/str.hnl` are
  **demoted to a backend** (not deleted): `kit-cli`/`kit-stdlib`/
  `kit-server`/`host-lsp` consume `str.*` via `plugin-lib-utils`'s
  `contribute` and don't wire the `char` resolver, so `StrModule` stays
  registered as the non-canonical fallback. The canonical `"str"` is
  `str-handl`.

## Stashes

- `stash@{0}`: "pre-ADR: result module refactoring" — a `plugin-lib-result`
  crate and related changes from a prior session. Untracked dirs
  `crates/plugin-lib-result/` and `handl-packages/result/` exist in the
  working tree but are NOT in the workspace `Cargo.toml`.
