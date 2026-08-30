# Agent Notes

## Build and test commands

Known, focused commands that pass:

```bash
cargo build
cargo check --workspace
cargo test -p handl-language
cargo test -p handl-emitters
cargo test -p handl-kit-data-canonical
cargo test -p handl-kit-canonical     # kit-canonical (legacy Kit wrapper)
cargo test -p handl-host-lsp
cargo test -p handl-host-tooling
cargo test -p handl-plugin-engine-eval-tree-walk
cargo test -p handl-plugin-engine-eval-cranelift
cargo test -p handl-plugin-engine-lex-pattern
cargo test -p handl-plugin-engine-parse-pratt
```

Broader checks (use when the change touches the whole workspace):

```bash
cargo test --workspace
```

## Architecture notes

The platform follows **ADR-0044: Language as Data and Runtime Split** (status:
Accepted). The language definition — tokens, grammar rules, AST shapes, eval
schemes — is pure serializable data, not Rust trait impls. Rust crates
_emit_ data fragments; kits _merge_ fragments into a `Language` value; engines
_interpret_ the `Language` data.

### Data-driven language pipeline

```
handl-emitter-*  →  handl-kit-data-canonical  →  handl-language::Language
(emit fragments)     (assemble + merge)            (pure data)
                                                    ↓
                              handl-runtime binds Language → engines
                                                    ↓
                    plugin-engine-lex-pattern      (lex)
                    plugin-engine-parse-pratt      (parse)
                    plugin-engine-eval-tree-walk   (eval: reference)
                    plugin-engine-eval-bytecode    (eval: bytecode)
                    plugin-engine-eval-cranelift   (eval: native)
```

### Key crates

- **`handl-language`** — The `Language` data schema. A `Language` is a
  mergeable store of typed fragment sets: `tokens` (`TokenKindDef`),
  `grammar` (`GrammarRow`), `eval` (`EvalSchemeDef`), `ast` (`AstShapeDef`),
  `caps` (`CapabilityDecl`), `options` (`OptionSchema`). Also defines `Token`
  (`{kind: String, text: String}`), `Source`, `Tokens`, `Ast`/`AstNode`,
  `Scheme` (eval combinators: `Lit`, `Var`, `Child`, `Bind`, `App`, `Branch`,
  `Prim`, `Foreign`), `FragmentSet<T>` (merge with declared-override policy),
  and the `Fragment` trait. Staged entry points: `lex`, `parse`, `eval`,
  `run`.

- **`handl-emitter-*`** (37 crates) — Each emits a small set of `Language`
  fragments for one language construct (e.g. `handl-emitter-let` emits the
  `let` token kind, grammar row, AST shape, and eval scheme). Emitters use
  helpers from **`handl-emitters`** (`declare_token`, `prefix_atom`,
  `prefix_compound`, `infix_binary`, `eval_scheme`, `prim_binary`,
  `app_call`). Emitters are the migration scaffold (ADR-0044 §6); the end
  state is authored data files.

- **`handl-kit-data-canonical`** — Assembles all 37 emitters into a
  `DataKit` / `Language`. Defines `EMITTER_NAMES`, `EXCLUDED_FRAGMENT_IDS`
  (3 fragments excluded to resolve behavioral collisions), `manifest()`,
  `registry()`, `assemble()`. This is the data-driven equivalent of the old
  `kit-canonical` plugin registry.

- **`handl-kit-data`** — The data kit framework: `DataKitManifest`,
  `FragmentSource` (emitter or data file), `FragmentSourceEntry`, include/
  exclude filters, `extends` chains.

- **`handl-runtime`** — Binds a `Language` to engine implementations.
  `Runtime<'h>` holds the `Language` + lex/parse/eval engines + host
  functions. `RuntimeConfig` selects engines by name (`with_lex`,
  `with_parse`, `with_eval`). Engine selection is data, not host-code linking.

- **`kit-canonical`** — Legacy `Kit` wrapper that provides `lex()`,
  `lex_with_spans()`, `parse()`, `eval()`, `run()` over the assembled
  `Language`. Also provides string-based token classification helpers
  (`is_identifier_kind`, `is_keyword_kind`, `is_number_kind`, `is_string_kind`,
  `is_bool_kind`, `is_null_kind`, `is_comment_kind`, `is_operator_kind`, etc.)
  and `SpannedToken`. Host crates use this instead of touching `Language`
  directly.

- **`handl-platform`** — The shared contract crate. Key types:
  `CapabilityId`, `CapabilityRequest`/`CapabilityResponse`, `Plugin` trait,
  `PluginRegistry`, `Kit`/`KitRegistry`, `Host`/`DefaultHost`,
  `LanguageHost`, `PlatformError`, `Boundary`, `AstNode`, `Span`.

- **`host-tooling`** — `ToolingHost<K>` owns a `Kit`, formatter, diagnostics,
  and LSP plugin slots (`CompletionPlugin`, `GotoDefPlugin`,
  `DocumentSymbolsPlugin`, `IncrementalSyncPlugin`). Semantic token
  classification uses string-based `token.kind` matching via `kit-canonical`'s
  `is_*_kind()` helpers (not typed `DynKind` — that system is deleted).

- **`host-lsp`** — LSP adapter: maps LSP requests to `ToolingHost` calls and
  converts results to `lsp-types`.

- **`host-cli`** — CLI host.

### Engine crates (`plugin-engine-*`)

- **`plugin-engine-lex`** — Lexer operation abstraction (trait, no engine).
- **`plugin-engine-lex-pattern`** — Data-driven pattern-matching lexer engine
  (reads `Language.tokens` + `Language.grammar` lexer rules).
- **`plugin-engine-parse`** — Parser operation abstraction.
- **`plugin-engine-parse-pratt`** — Pratt parser engine reading
  `Language.grammar` data rows.
- **`plugin-engine-eval`** — Eval operation abstraction.
- **`plugin-engine-eval-tree-walk`** — Reference interpreter: interprets
  `Scheme` combinators directly.
- **`plugin-engine-eval-bytecode`** — Compiles `Scheme` to bytecode, executes
  via VM.
- **`plugin-engine-eval-cranelift`** — Compiles `Scheme` to native code via
  cranelift. Supports on-disk binaries via `cranelift-object`.

### Fragment merge policy (ADR-0044 §5)

`FragmentSet<T>` merge distinguishes **override** from **collision**:

- **Override** = fragment declares `replaces: [existing_id]`. Allowed,
  auditable.
- **Collision** = two fragments target the same identity key, neither
  declares `replaces`. **Error.**
- **Union** = `TokenKindDef` and `AstShapeDef` have `UNION = true`:
  duplicate structural declarations merge idempotently.
- `GrammarRow` and `EvalSchemeDef` remain non-union: behavioral conflicts are
  detected as errors.

### Plugins

Plugins live in `crates/plugin-*` and are registered into `PluginRegistry` /
kit manifests. `CapabilityRequest` is the `handl-platform` request envelope:
a `CapabilityId` plus a `serde_json::Value` payload. `Plugin::run` receives
it and a `PluginContext`.

## Conventions

- New crates must be added to `Cargo.toml` `[workspace.members]` and
  `[workspace.dependencies]`.
- Prefer `PlatformError` for plugin/runtime failures; do not leak concrete
  crate errors into `handl-platform` APIs.
- New language constructs should be added as a `handl-emitter-*` crate that
  emits data fragments, not as a Rust `impl Behavior`. See ADR-0044 §6.
- Run `vp check` before committing. Run `vp check --fix` to auto-fix
  formatting.

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
- Outputs go to `coverage/` (gitignored). CI runs in `.github/workflows/coverage.yml`.
