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
- `handl-expr` defines the pluggable parser. Entry points: `ExpressionPipeline`, `ModulePipeline`. `Stmt { kind: StmtKind, leading, trailing }` wraps a statement; `StmtKind` covers `Expr`, `Let`, `Const`, `Import`, `Export`, `Module`. `Expr<T>` lives in `handl-ast-canonical`; `Expr<NonFunctionValue>` (from `handl-vm-values-json`) is the canonical expression type.
- `handl-kit-canonical` provides `CanonicalKit` and `CanonicalHost`. `CanonicalKit` composes a `ModulePipeline` and optional `TypecheckPlugin` / `RuntimePlugin`. Use it for quick parse/typecheck/eval tests.
- `handl-tooling` provides `ToolingHost`, which owns a `CanonicalKit`, a `Formatter`, and a `Diagnostics` implementation and exposes slots for LSP plugins: `CompletionPlugin`, `GotoDefPlugin`, `DocumentSymbolsPlugin`, `IncrementalSyncPlugin`.
- `handl-host-lsp` is the LSP adapter: it maps LSP requests to `ToolingHost` calls and converts results to `lsp-types`.
- Plugins live in `crates/plugin-*` and are registered into `PluginRegistry` / kit manifests.
- `CapabilityRequest` is the `handl-platform` request envelope: a `CapabilityId` plus a `serde_json::Value` payload. `Plugin::run` receives it and a `PluginContext`.

## Conventions

- New crates must be added to `Cargo.toml` `[workspace.members]` and `[workspace.dependencies]`.
- Plugin crates are named `crates/plugin-*` and register fragments by id through `CanonicalHost` or `LanguageHost`.
- Prefer `PlatformError` for plugin/runtime failures; do not leak concrete crate errors into `handl-platform` APIs.
