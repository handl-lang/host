# ADR-0105: The Typechecker is Handl, Not Rust

## Status

Proposed.

## Context

ADR-0103 established that the kernel is generic evaluator/VM infrastructure,
not the calculus. Five Handl-authored models were created: `exec_model.hnl`,
`arrow_model.hnl`, `type_model.hnl`, `meta_model.hnl`, and `value_model.hnl`.
The `Term` → `TermData` migration is in progress (4,311 → 3,086 references
migrated so far).

Three Rust crates currently own typechecking:

| Crate | Lines | Role |
|-------|-------|------|
| `plugin-engine-typecheck` | ~1,500 | `TypecheckEngine` trait + `bootstrap.rs` (scope/import checker) |
| `plugin-engine-typecheck-nbe` | ~5,500 + 11,364 tests | NBE engine: AST walk, context, unification, dependent types |
| `plugin-lib-lang-typecheck` | ~229 | Host function `typecheck.typecheck(lang, ast)` |

`type_model.hnl` (459 lines) covers HoTT tag-to-judgment interpretation but
not the surrounding driver: AST traversal, typing context Γ, unification,
metavariables, dependent-type closures, predicate evaluation, totality
checking, or module-level scope checks.

The bootstrap problem: `kit-canonical` hard-wires
`NbeTypecheckEngine::new(...)` at compile time. But the tree-walk eval
engine is already available when the kit is built, so a Handl typecheck
model can be loaded as a `.hnl` module and interpreted by the tree-walk
engine — no precompilation or two-stage bootstrap needed.

Key finding: `hnl typecheck` currently uses `bootstrap.rs`
(`check_module_with_prelude`), NOT the NBE `TypecheckEngine`. `hnl run`/
`eval`/`test` skip typechecking entirely. The NBE engine is wired into
`Runtime::typecheck` / `Runtime::run_checked` / `Kit::run` but not
exercised by the CLI's normal paths. This means the migration can proceed
without breaking the dogfood gate.

## Decision

Delete all three typecheck Rust crates. Replace with Handl-authored
typechecking models. Keep only a thin Rust `TypecheckEngine` trait and a
`HandlTypecheckEngine` adapter that delegates to the Handl models.

### Architecture

```
Kernel (Rust, irreducible):
  - TypecheckEngine trait (the contract)
  - HandlTypecheckEngine adapter (marshals Language + Ast, calls model)
  - Runtime registration (resolve engine by name)

Typechecking (Handl, compiled to bytecode):
  - typecheck_model.hnl   — the driver: AST walk, Γ, unification, dispatch
  - type_model.hnl        — HoTT tag-to-judgment interpretation (exists)
  - scope_model.hnl       — module-level scope/import/export checks
  - totality_model.hnl    — termination/totality checking
```

### What stays in Rust

- `TypecheckEngine` trait (~30 lines) — the Rust contract that
  `handl-runtime` dispatches over. Moves to `plugin-engine` or
  `handl-runtime` itself.
- `TypecheckRequest` / `TypecheckResult` / `TypecheckDiagnostic` —
  the data types crossing the Rust ↔ Handl boundary.
- `HandlTypecheckEngine` — a thin adapter that:
  1. Loads the typecheck model(s) from embedded `.hnl` source.
  2. Marshals `Language` (as `Value`) and `Ast` (as `Value`) into the
     model's expected format.
  3. Calls the model's `typecheck(lang, ast)` function via the tree-walk
     eval engine.
  4. Unmarshals `TypecheckResult` from the returned `Value`.
- `TypecheckFn` in `plugin-lib-lang-typecheck` — the host function
  behind `typecheck.typecheck(lang, ast)`. Unchanged: it already calls
  `handle.typecheck(&ast)`.

### What moves to Handl

| NBE component | Handl replacement |
|---------------|-------------------|
| `eval_type_scheme` (tag dispatch) | `type_model.hnl` `run` (exists) |
| `eval_judgment` (~50 ops) | `typecheck_model.hnl` judgment ops |
| `synth_node` / `check_node` (AST walk) | `typecheck_model.hnl` synth/check |
| Γ management (extend, lookup, bind) | `typecheck_model.hnl` context ops |
| Unification + metavariables | `typecheck_model.hnl` unify |
| Dependent-type closures (Pi/Sigma/Forall/Vau) | `typecheck_model.hnl` |
| Predicate evaluation | `typecheck_model.hnl` (or `predicate_model.hnl`) |
| Totality checking | `totality_model.hnl` |
| `bootstrap.rs` scope/import/export checks | `scope_model.hnl` |

### Bootstrap solution

No two-stage bootstrap needed. The tree-walk eval engine
(`TreeWalkEngine`) is instantiated in `kit-canonical` before the
typecheck engine is registered. The `HandlTypecheckEngine` adapter:

1. Embeds the `.hnl` model source at compile time (`include_str!`).
2. At registration time, parses + loads the model into a `HandlModule`.
3. When `typecheck()` is called, invokes the model's `typecheck` function
   via the tree-walk engine.

This works because the tree-walk engine does not depend on the
typechecker — it depends only on `plugin-engine-exec` (the eval
abstraction) and `handl-language` (the `Language`/`Ast` types).

## Consequences

### Positive

- Typechecking logic becomes data, not Rust. Adding a type rule is a
  data change (YAML + Handl model), not an engine recompile.
- The HoTT/type theory is fully auditable as Handl code.
- The 5,500-line NBE engine + 1,081-line bootstrap + 1,076-line
  predicate evaluator + 407-line totality checker = ~8,064 lines of
  Rust deleted.
- The `Term` enum loses its largest consumer (1,248 references).
- ADR-0103 reaches its end state for the typechecker.

### Negative

- Performance: the typechecker runs on the tree-walk VM instead of
  native Rust. This may be slower for large modules. Mitigation: the
  bytecode/cranelift engines can compile the typecheck model once
  the VM is mature.
- The 11,364 lines of NBE tests need to be ported to test the Handl
  models. Many can be expressed as Handl-level parity tests; some
  may need new test infrastructure.
- Risk of semantic drift during migration. Mitigation: keep the Rust
  NBE engine until parity tests pass, then delete.

### Neutral

- `plugin-lib-lang-typecheck` stays (it's a thin host function).
- The `TypecheckEngine` trait stays (it's the Rust contract).
- `canonical.yaml`'s `runtime.typecheck: nbe` stays (the name is
  resolved to whatever engine is registered).

## Migration Plan

### Phase 1: Author the Handl typecheck models

1. **`typecheck_model.hnl`** — the driver:
   - `typecheck(lang, ast)` entry point
   - AST walk via `lang.types[node.kind]` → `TypeRuleDef` → `synth`/`check`
   - Γ context management (extend, lookup, bind, result)
   - Judgment ops (~50): `eval_child`, `proj_child`, `unify`, `bind`,
     `result`, `eval_scheme`, `fresh_meta`, `solve_meta`, etc.
   - Dependent-type closures: Pi, Sigma, Forall, Vau
   - Unification + metavariable store
   - Delegates tag interpretation to `type_model.hnl`

2. **`scope_model.hnl`** — module-level checks (replaces `bootstrap.rs`):
   - Collect bindings (imports, exports, `name = expr`)
   - Detect duplicates
   - Unbound-wire detection
   - Unused-import detection
   - `rec` region well-formedness

3. **`totality_model.hnl`** — termination checking:
   - `check_totality(ast, expr_ref, lang)`
   - Decreasing argument/field detection
   - Measure-based termination

4. Extend `type_model.hnl` if needed for judgment-level HoTT ops
   not yet covered.

### Phase 2: Write the `HandlTypecheckEngine` adapter

1. Create `HandlTypecheckEngine` in a new thin crate or in
   `plugin-engine`:
   - `new(eval_engine: Box<dyn EvalEngine>) -> Self`
   - Embeds model source via `include_str!`
   - Loads models into `HandlModule` at construction
   - `typecheck(&self, lang, req)` marshals `Language` + `Ast` to
     `Value`, calls model, unmarshals `TypecheckResult`
   - Implements `TypecheckEngine` trait

2. Move `TypecheckEngine` trait + `TypecheckRequest`/`Result`/`Diagnostic`
   to `plugin-engine` or `handl-runtime` (they're small data types).

### Phase 3: Wire and verify

1. Register `HandlTypecheckEngine` in `kit-canonical` under `"nbe"`.
2. Route `hnl typecheck` through `Kit::typecheck` instead of
   `check_module_with_prelude`.
3. Port NBE tests as Handl-level parity tests.
4. Run the dogfood gate (`hnl fmt --check && hnl lint && hnl typecheck
   && hnl test`).
5. Verify all 177 corpus files still pass.

### Phase 4: Delete the Rust crates

1. Delete `crates/plugin-engine-typecheck-nbe/` (the NBE engine).
2. Delete `crates/plugin-engine-typecheck/` (the bootstrap checker).
   - Move `TypecheckEngine` trait to `plugin-engine` or `handl-runtime`
     first.
3. Update `kit-canonical` to use `HandlTypecheckEngine`.
4. Update `host-cli` to use `Kit::typecheck` instead of
   `check_module_with_prelude`.
5. Update `Cargo.toml` workspace members.
6. Update all downstream `Cargo.toml` dependencies.

### Phase 5: Update ADR-0103

Mark ADR-0103's typechecker migration as complete. Update reference
counts. Update the ADR-0105 status to Accepted.

## Open Questions

1. **Performance**: Is the tree-walk engine fast enough for
   typechecking 177-file corpus? If not, can the bytecode engine
   compile the typecheck model?
2. **Test porting**: The 11,364 lines of NBE tests cover many edge
   cases. How many can be directly ported to Handl-level tests vs.
   needing new test infrastructure?
3. **`plugin-lib-lang-typecheck`**: Does this crate stay as-is, or
   does it also get absorbed into the Handl model? (Currently it's
   a thin host function — likely stays.)
4. **Type registry**: `NbeTypecheckEngine` uses
   `handl-plugin-lib-tag::TypeRegistry` to mint type values. Does
   this stay as a Rust host primitive, or does the type model
   construct type values directly?
