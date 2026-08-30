# Agent Notes

## Build and test commands

Known, focused commands that pass:

```bash
cargo build
cargo check --workspace
cargo test -p handl-kit-canonical
cargo test -p handl-platform
cargo test -p handl-host-lsp
cargo test -p handl-expr
```

Broader checks (use when the change touches the whole workspace):

```bash
cargo test -p handl-platform -p handl-kit-canonical -p handl-expr -p handl-host-lsp
cargo test --workspace
```

## Architecture notes

- `handl-platform` is the only shared contract crate. Key types: `CapabilityId`, `CapabilityRequest`/`CapabilityResponse`, `Plugin` trait, `PluginRegistry`, `Kit`/`KitRegistry`, `Host`/`DefaultHost`, `LanguageHost`, `PlatformError`, `Boundary`.
- `handl-expr` defines the pluggable parser. Entry points: `ExpressionPipeline`, `ModulePipeline`. `Stmt { kind: StmtKind, leading, trailing }` wraps a statement; `StmtKind` covers `Expr`, `Let`, `Const`, `Import`, `Export`, `Module`. `Expr<T>` lives in `handl-ast-canonical`; `Expr<NonFunctionValue>` (from `handl-plugin-vm-values-json`) is the canonical expression type.
- `handl-tooling` provides `ToolingHost`, which owns a `CanonicalKit`, a `Formatter`, and a `Diagnostics` implementation and exposes slots for LSP plugins: `CompletionPlugin`, `GotoDefPlugin`, `DocumentSymbolsPlugin`, `IncrementalSyncPlugin`.
- `handl-host-lsp` is the LSP adapter: it maps LSP requests to `ToolingHost` calls and converts results to `lsp-types`.
- Plugins live in `crates/plugin-*` and are registered into `PluginRegistry` / kit manifests.
- `CapabilityRequest` is the `handl-platform` request envelope: a `CapabilityId` plus a `serde_json::Value` payload. `Plugin::run` receives it and a `PluginContext`.

## Conventions

- New crates must be added to `Cargo.toml` `[workspace.members]` and `[workspace.dependencies]`.
- Prefer `PlatformError` for plugin/runtime failures; do not leak concrete crate errors into `handl-platform` APIs.

## Coverage

The repo uses LLVM source-based coverage via `cargo-llvm-cov` (install with
`cargo install cargo-llvm-cov`). It requires the `llvm-tools-preview` rustup
component. Cargo aliases are defined in `.cargo/config.toml`; the runner script
is `scripts/coverage.sh`.

```bash
cargo cov                 # terminal summary of per-crate coverage
cargo cov-html            # HTML report under coverage/html/ (opens browser)
cargo cov-lcov            # lcov.info at coverage/lcov.info (for IDEs/CI)
cargo cov-xml             # Cobertura XML at coverage/cobertura.xml
cargo cov-clean           # discard stale instrumentation artifacts

scripts/coverage.sh                       # HTML + lcov + summary
scripts/coverage.sh --summary             # summary only, no files
scripts/coverage.sh --doctests            # include doctests
scripts/coverage.sh -p handl-platform     # single crate
scripts/coverage.sh --fail-under 80       # exit 1 if line coverage < 80%
scripts/coverage.sh --clean               # clean stale artifacts first
```

Notes:

- Coverage builds disable incremental compilation (`CARGO_INCREMENTAL=0`).
- The `fuzz/` workspace is separate and not covered by `--workspace`.
- Outputs go to `coverage/` (gitignored). CI runs in `.github/workflows/coverage.yml`.
