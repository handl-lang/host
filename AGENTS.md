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

| Concern | Crate | Key API |
|---|---|---|
| Shared diagnostics | `plugin-ext-diag` | `Diagnostic`, `Severity`, `Span`, `Fix`, `render_diagnostic`, `apply_fixes` |
| Formatter | `plugin-ext-fmt` | `format_source(src)`, `check_source(src)` |
| Linter | `plugin-ext-lint` | `lint_source(src)`, `check_source(src)`, `fix_source(src)` |
| Typechecker | `plugin-engine-typecheck` | `check_module(src) -> TypecheckReport` |
| Test runner | `plugin-lib-test` | `discover(paths)`, `run(plan)`, `run_with_filter(plan, filter)` |
| CLI | `host-cli` | `hnl fmt/lint/check/test/run/eval` subcommands |

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
would create a cycle. `plugin-ext-fmt` and `plugin-lib-test` use
`handl-kit-data-canonical` (no eval engine) or `handl-kit-canonical`
respectively.

### Source splitting

The Pratt parser parses one statement per call. The typechecker, test runner,
and linter all split source into top-level statements at bracket-depth zero
(respecting strings and `--` comments) before parsing each chunk individually.

### Known issues

- `handl-defining-handl` cranelift bootstrap test fails on `let` bindings
  (pre-existing, unrelated to ADR 0085-0091 work).
- `plugin-engine-parse-pratt` has an unused import warning (`PayloadTokenSpec`).

## Stashes

- `stash@{0}`: "pre-ADR: result module refactoring" — a `plugin-lib-result`
  crate and related changes from a prior session. Untracked dirs
  `crates/plugin-lib-result/` and `handl-packages/result/` exist in the
  working tree but are NOT in the workspace `Cargo.toml`.
