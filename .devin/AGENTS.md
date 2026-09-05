# Agent Notes

## Build and test commands

Known, focused commands that pass:

```bash
cargo build
cargo check --workspace
cargo test -p handl-language
cargo test -p handl-kit-data-canonical
cargo test -p handl-kit-canonical     # kit-canonical (legacy Kit wrapper)
cargo test -p handl-host-lsp
cargo test -p handl-host-tooling
cargo test -p handl-plugin-engine-eval-tree-walk
cargo test -p handl-plugin-engine-eval-bytecode   # v1 + v2 bytecode VM
cargo test -p handl-plugin-engine-eval-cranelift
cargo test -p handl-plugin-engine-lex-pattern
cargo test -p handl-plugin-engine-parse-pratt
cargo test -p handl-plugin-lib-arrow-iter          # v2 derived iteration patterns
```

Broader checks (use when the change touches the whole workspace):

```bash
cargo test --workspace
```

## Architecture notes

The platform follows **ADR-0044: Language as Data and Runtime Split** (status:
Accepted, §6 end state complete). The language definition — tokens, grammar
rules, AST shapes, eval schemes — is pure serializable data (YAML files), not
Rust trait impls. Kits _merge_ fragments into a `Language` value; engines
_interpret_ the `Language` data.

### Data-driven language pipeline

```
handl-kit-data-canonical/data/*.yaml  →  handl-language::Language
  (7 YAML files, one per fragment type)    (pure data, deserialized at
                                            compile time via include_str!)
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

- **`handl-kit-data-canonical`** — The canonical language definition. Loads
  7 YAML data files (`data/tokens.yaml`, `data/grammar.yaml`, `data/eval.yaml`,
  `data/ast.yaml`, `data/caps.yaml`, `data/options.yaml`,
  `data/lexer_rules.yaml`) embedded via `include_str!` and deserializes them
  into a `Language`. Defines `EXCLUDED_FRAGMENT_IDS` (3 fragments excluded to
  resolve behavioral collisions), `manifest()`, `registry()`, `assemble()`.
  This is the data-driven equivalent of the old `kit-canonical` plugin
  registry. The 37 `handl-emitter-*` crates that originally produced these
  files have been deleted (ADR-0044 §6 end state).

- **`handl-emitters`** — Thin trait definition crate (`Emitter` trait) used
  by the kit assembly framework. No per-construct emitters exist; the
  language definition is the YAML files in `handl-kit-data-canonical/data/`.

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

### Arrow-calculus v2 kernel (ADR-0052, in progress)

ADR-0052 swaps the eval kernel from the v1 lambda-flavored `Scheme`
(`Lit`/`Var`/`Child`/`Bind`/`App`/`Branch`/`Prim`/`Foreign`/`Vau`) to a
six-primitive arrow calculus (`arr`/`>>>`/`first`/`choice`/`loop`/`app`).
The migration is **additive so far** — v1 is untouched and remains the
canonical representation for v1 `Language` values; v2 lives alongside it.

- **`handl-language::scheme_v2`** — `SchemeV2` enum
  (`Arr`/`Seq`/`First`/`Choice`/`Loop`/`App`) + `ArrSource`
  (`Native`/`Foreign(name)`). `Arr` carries an optional `lit: Option<Value>`
  payload for ops that take a literal operand (`const`/`proj_env`/
  `proj_child`/`extend_env`) — this concretizes ADR-0052's loose
  `args: [v]` sketch so `args` stays a pure sub-scheme list. Constructors:
  `arr_native`/`arr_foreign`/`arr_native_lit`/`seq`/`first`/`choice`/
  `loop_feedback`/`app`. `lower_v1_to_v2(&Scheme) -> Result<SchemeV2,
LangError>` implements the ADR lowering table (Vau → error; v1 `App` →
  dynamic `app` form). `foreign_names_v2` derives capabilities from v2.
  Re-exported at the crate root: `SchemeV2`, `ArrSource`,
  `lower_v1_to_v2`, `foreign_names_v2`.
- **`handl-language::version`** — `SchemeVersion::v2()`; `supports()` is
  backward-compatible: a v2 engine accepts v1 languages; v1 engine rejects
  v2 (a compile-time error per ADR-0052).
- **`plugin-engine-eval-tree-walk::v2`** — `V2Interpreter`, a reference
  **handler** that interprets `SchemeV2` directly (Sanada's model:
  execution is one handler interpretation). Value shapes: pairs are
  `{"__pair__": [a, b]}` (`First`); Eithers are
  `{"__either__": "left"|"right", "value": v}` (`Choice`, produced by the
  `tag` op which is the `Branch` lowering). `Loop` supports **both**
  single-step (body returns a pair `(Out, State)` → returns `Out`) and
  **multi-step** iteration (body returns an Either: `left v` terminates
  with `v`; `right s` continues with new state `s`). This resolves
  ADR-0052 open question #1. `eval_lowered` runs a v1 `Language`
  end-to-end by lowering each scheme v1→v2 then interpreting. Reuses the
  v1 `ScopeStack` (made `pub`) and v1 closure/`$payload` conventions.
- **`plugin-lib-arrow-iter`** — Derived iteration-pattern library over the
  multi-step `Loop` semantics: `while_loop`/`for_loop`/`fix_loop`
  constructors built from `Loop` + `Choice`. Implements the ADR-0052
  Phase E library patterns.
- **`plugin-engine-eval-bytecode` v2 modules** — `instr_v2` (the v2 ISA:
  `Push`/`Load`/`LoadPayloadVar`/`LoadPayload`/`EvalChild`/`Store`/
  `StorePayloadName`/`PopScope`/`Call`/`ChoiceJump`/`Jump`/`NativeOp`/
  `ForeignOp`/`FirstOp`/`Pair`/`LoopCheck`/`Return`), `compile_v2`
  (`SchemeV2` → `Vec<InstrV2>`; special-cases lit-bearing ops
  `const`/`proj_env`/`proj_child`/`extend_env`), `vm_v2` (`VmFrameV2`
  executing the v2 ISA with a per-node compilation cache, scope stack,
  and a 100k-iteration loop guard). `BytecodeEngineV2` implements
  `EvalEngine` with `SchemeVersion::v2().supports(...)` (accepts v1 and
  v2 langs). `First` compiles to a `FirstOp(start, end)` sub-program
  instruction so inner arrows like `const` (which don't consume the
  incoming value) work correctly on a fresh operand stack. **Limitation:**
  the derived step ops `while_step`/`for_step`/`fix_step` are not yet
  supported by the v2 bytecode VM (they require recursive sub-program
  execution from within a native op); users construct equivalent behavior
  with `Loop` + `Choice` directly. The v2 tree-walk handler supports them.
- **`plugin-engine-eval-cranelift` v2 module** — `compile_v2.rs` +
  `CraneliftEngineV2`. Compiles `SchemeV2` to native code via cranelift
  JIT. v2 functions have signature `fn(*mut EvalCtx, *mut Value) ->
*mut Value` (ctx, incoming) → result — the incoming value is a
  parameter, not stored in `EvalCtx`. v1→v2 lowering happens at compile
  time (baked into IR). `Choice` → native `brif`/`jump` with merge block;
  `Loop` → native back-edge blocks with stack-slot iteration counter and
  3-way exit (left/right/pair); `First` → inline `unpair_fst`/`unpair_snd`
  - compile inner + re-pair (no sub-program needed — cranelift has native
    control flow). All v2 helpers are `helper_v2_*` (separate from v1
    `helper_*`). Same derived-step-op limitation as bytecode v2.

Conformance tests: `crates/handl-language/tests/lowering_conformance.rs`
(every lowering-table row) and
`crates/handl-kit-data-canonical/tests/lowering.rs` (all 52 `eval.yaml`
schemes lower; only `eval.vau` errors).

**Remaining cutover work (the risky big-bang, NOT yet done):**

1. Make v2 the default kernel; delete v1 `Scheme` once parity is proven on
   the full workspace suite (ADR-0052 Phase A completion). **Parity is
   proven**: `crates/handl-defining-handl/tests/v2_parity.rs` runs all 42
   self-hosting corpus cases through all six engines (v1 + v2 ×
   tree-walk/bytecode/cranelift) — all agree. The cutover is unblocked.
2. Rewrite `data/eval.yaml` to v2 schemes per the lowering table; remove
   `eval.assign`/`eval.stmt.let`/`eval.stmt.const` (folded into `x = y`).
3. Update `plugin-engine-eval-cranelift` to interpret `SchemeV2`
   (Phase B) — **done additively** (`compile_v2.rs` + `CraneliftEngineV2`
   with v2 helpers; v1 compiler untouched). Both bytecode and cranelift
   v2 engines are complete.
4. `proc x -> ...` surface grammar + desugar to the six combinators
   (Phase C) — **done additively** (token + grammar rule + AST shape +
   `!Child 0` eval scheme in `handl-kit-data-canonical`); `function`/
   `call` remain as v1 compatibility.
5. Bootstrap rewrite in arrow calculus / `proc` (Phase D) — **done
   additively**: `proc_corpus()` in `handl-defining-handl` (5 proc-authored
   cases), `data/handl-defining-handl-proc.hnl` (proc-authored bootstrap),
   `tests/futamura_v2.rs` (v2 Futamura projection tests with
   `CraneliftEngineV2`). All engines pass the proc corpus.
6. `while`/`for`/`fix` as library patterns over `Loop` + `Choice`
   (Phase E) — **done** in `plugin-lib-arrow-iter` (multi-step `Loop`
   semantics resolve open question #1).

### `Vau` handler concern (ADR-0052)

`Vau` is **removed from the v2 kernel** — it is not one of the six arrow
combinators (`Arr`/`Seq`/`First`/`Choice`/`Loop`/`App`). The v2 eval YAML
(`data/eval_v2.yaml`) intentionally omits `eval.vau`. `lower_v1_to_v2`
returns an error for `Scheme::Vau`.

Fexpr-style behaviors (short-circuiting `and`, pattern matching, macros,
exception handling) are a **handler/evaluator concern**, not a calculus
combinator. They live in the handler layer (the engine's native op set),
not in the `SchemeV2` enum. The v2 engines can be extended with these ops
without touching the calculus.

**Current state of fexpr-style ops in v2:**

- `match`, `handle`, `json` — v1 `Prim` ops that are node-aware (read from
  `ctx.current_node.payload`). NOT yet implemented in the v2 native op
  set. These are handler extensions to add when needed.
- `array`, `object`, `index`, `assign`, `typeof`, `pow`, `compare` —
  **done** (added to all three v2 engines for parity).
- Short-circuiting `and`/`or` — currently strict (evaluate both args).
  Short-circuiting would be a handler extension (a v2 native op that
  delays evaluation of the second arg).

The v1 `Scheme::Vau` variant and v1 engine implementations (`VauEnter`/
`VauBind` in bytecode, `helper_vau_enter`/`helper_vau_bind` in cranelift)
remain in place during the additive phase. They will be removed in the
big-bang cutover (item 1 above).

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
- New language constructs should be added as YAML fragments in
  `handl-kit-data-canonical/data/` (one file per fragment type), not as Rust
  `impl Behavior`. See ADR-0044 §6.
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
