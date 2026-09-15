# ADR-0105: The Typechecker is Handl, Not Rust

## Status

Accepted — Phases 1–2 complete, Phase 3 in progress, Phase 4 pending,
Phase 5 in progress (this update). The Handl typecheck models are
authored and parse; the `HandlTypecheckEngine` adapter compiles and
passes 23 tests; the adapter is wired into `kit-canonical`. The models
are skeletons — full typechecking is not yet working end-to-end. The
Rust typecheck crates are NOT deleted (pending production-ready models).

## Context

ADR-0103 established that the kernel is generic evaluator/VM infrastructure,
not the calculus. Five Handl-authored models were created: `exec_model.hnl`,
`arrow_model.hnl`, `type_model.hnl`, `meta_model.hnl`, and `value_model.hnl`.
The `Term` → `TermData` migration is in progress (4,311 → ~3,086 references
migrated so far; 1,225 migrated across bytecode compiler, tree-walk engine,
Cranelift compiler, refinement engine (partial), `handl-language` (partial),
NBE typechecker (`lib.rs`), `handl-runtime` (tests), `plugin-engine`).

Three Rust crates currently own typechecking:

| Crate                         | Lines                 | Role                                                            |
| ----------------------------- | --------------------- | --------------------------------------------------------------- |
| `plugin-engine-typecheck`     | ~1,500                | `TypecheckEngine` trait + `bootstrap.rs` (scope/import checker) |
| `plugin-engine-typecheck-nbe` | ~5,500 + 11,364 tests | NBE engine: AST walk, context, unification, dependent types     |
| `plugin-lib-lang-typecheck`   | ~229                  | Host function `typecheck.typecheck(lang, ast)`                  |

`type_model.hnl` (817 lines, extended from 459) covers HoTT
tag-to-judgment interpretation — now with 26 new judgment ops
(unification, metavariables, transport, and `Src`/`Tgt`/`CellId`/
`CellCompV`/`Whisker`/`Univalence`/`Glue`/`Fibrant`/`JElim`/
`RecordProj`/`RecordWith` tag arms) — but not the surrounding driver:
AST traversal, typing context Γ, unification, metavariables,
dependent-type closures, predicate evaluation, totality checking, or
module-level scope checks. Those are the job of the three new models
authored in Phase 1 (below).

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

| NBE component                                 | Handl replacement                                |
| --------------------------------------------- | ------------------------------------------------ |
| `eval_type_scheme` (tag dispatch)             | `type_model.hnl` `run` (exists)                  |
| `eval_judgment` (~50 ops)                     | `typecheck_model.hnl` judgment ops               |
| `synth_node` / `check_node` (AST walk)        | `typecheck_model.hnl` synth/check                |
| Γ management (extend, lookup, bind)           | `typecheck_model.hnl` context ops                |
| Unification + metavariables                   | `typecheck_model.hnl` unify                      |
| Dependent-type closures (Pi/Sigma/Forall/Vau) | `typecheck_model.hnl`                            |
| Predicate evaluation                          | `typecheck_model.hnl` (or `predicate_model.hnl`) |
| Totality checking                             | `totality_model.hnl`                             |
| `bootstrap.rs` scope/import/export checks     | `scope_model.hnl`                                |

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

### Phase 1: Author the Handl typecheck models — COMPLETE

All four models authored and parse successfully:

1. **`typecheck_model.hnl`** (1,305 lines) — the driver:
   - `typecheck(lang, ast)` entry point
   - AST walk via `lang.types[node.kind]` → `TypeRuleDef` → `synth`/`check`
   - Γ context management (extend, lookup, bind, result)
   - Judgment ops (~50): `eval_child`, `proj_child`, `unify`, `bind`,
     `result`, `eval_scheme`, `fresh_meta`, `solve_meta`, etc.
   - Dependent-type closures: Pi, Sigma, Forall, Vau
   - Unification + metavariable store
   - Delegates tag interpretation to `type_model.hnl`

2. **`scope_model.hnl`** (801 lines) — module-level checks (replaces
   `bootstrap.rs`):
   - Collect bindings (imports, exports, `name = expr`)
   - Detect duplicates
   - Unbound-wire detection
   - Unused-import detection
   - `rec` region well-formedness

3. **`totality_model.hnl`** (718 lines) — termination checking:
   - `check_totality(ast, expr_ref, lang)`
   - Decreasing argument/field detection
   - Measure-based termination

4. **`type_model.hnl`** extended from 459 to 817 lines with 26 new
   HoTT judgment ops (unification, metavariables, transport, and
   `Src`/`Tgt`/`CellId`/`CellCompV`/`Whisker`/`Univalence`/`Glue`/
   `Fibrant`/`JElim`/`RecordProj`/`RecordWith` tag arms).

**Caveat:** the models parse and load, but they are skeletons — the
judgment ops and AST-walk logic are structurally complete but not yet
semantically validated against the Rust NBE engine. Full typechecking
is not yet working end-to-end.

### Phase 2: Write the `HandlTypecheckEngine` adapter — COMPLETE

1. `HandlTypecheckEngine` created in a new thin crate
   `plugin-engine-typecheck-handl`:
   - `new(eval_engine: Box<dyn EvalEngine>) -> Self`
   - Embeds model source via `include_str!`
   - Loads models into `HandlModule` at construction, wiring stdlib
     `array`/`hash_map`/`typeof`/`tag` dependencies
   - `typecheck(&self, lang, req)` marshals `Language` + `Ast` to
     `Value`, calls model, unmarshals `TypecheckResult`
   - Implements `TypecheckEngine` trait
   - Graceful degradation: if the model fails to load, returns
     `LangError::NotImplemented` (the crate always compiles)

2. `TypecheckEngine` trait + `TypecheckRequest`/`Result`/`Diagnostic`
   remain in `plugin-engine-typecheck` (not yet moved).

**Test status:** 23 tests pass (6 unit in `lib.rs` + 14 in
`handl_typecheck_parity.rs` + 3 in `integration.rs`).

### Phase 3: Wire and verify — IN PROGRESS

1. ✅ `HandlTypecheckEngine` registered in `kit-canonical` under the
   `"nbe"` name (alongside the existing `NbeTypecheckEngine`).
2. ⏳ `hnl typecheck` still routes through
   `check_module_with_prelude` (not yet switched to `Kit::typecheck`).
3. ⏳ NBE tests not yet ported as Handl-level parity tests.
4. ⏳ Dogfood gate testing in progress — the adapter is wired but the
   models return errors when called (graceful degradation); the gate
   still passes because `hnl typecheck` uses the bootstrap path.
5. ⏳ 177 corpus files not yet verified through the Handl models.

**Stdlib wiring fix:** the adapter now wires `array`, `hash_map`,
`typeof`, and `tag` modules as dependencies of the typecheck model
(via `with_dependency` in `HandlTypecheckEngine::new`). Without this,
the model failed to load because it calls `array.first`, `hash_map.get`,
`typeof.type_of`, and `tag.tag` at runtime.

### Phase 4: Delete the Rust crates — PENDING

Cannot proceed until the Handl models are production-ready (full
typechecking working end-to-end with parity against the NBE engine).

1. Delete `crates/plugin-engine-typecheck-nbe/` (the NBE engine).
2. Delete `crates/plugin-engine-typecheck/` (the bootstrap checker).
   - Move `TypecheckEngine` trait to `plugin-engine` or `handl-runtime`
     first.
3. Update `kit-canonical` to use `HandlTypecheckEngine`.
4. Update `host-cli` to use `Kit::typecheck` instead of
   `check_module_with_prelude`.
5. Update `Cargo.toml` workspace members.
6. Update all downstream `Cargo.toml` dependencies.

### Phase 5: Update ADRs — IN PROGRESS (this update)

ADR-0103 updated with the new models, adapter, Term:: reference counts,
and ADR-0105 cross-reference. ADR-0105 status changed from Proposed to
Accepted. ADR-0103's typechecker migration will be marked complete once
Phase 4 (delete Rust crates) is done.

## Current state

### What works

- All four models (`typecheck_model.hnl`, `scope_model.hnl`,
  `totality_model.hnl`, extended `type_model.hnl`) parse and load via
  the `HandlModule` loader.
- `HandlTypecheckEngine` adapter compiles and implements the
  `TypecheckEngine` trait.
- 23 tests pass: model loading, type-model tag interpretation,
  typecheck-driver smoke tests, totality-model smoke tests, and
  integration tests (adapter construction + typecheck invocation).
- The adapter is wired into `kit-canonical` (registered under `"nbe"`).
- Stdlib dependencies (`array`, `hash_map`, `typeof`, `tag`) are wired
  into the adapter so the model loads successfully.

### What doesn't work yet

- **Models are skeletons.** The judgment ops and AST-walk logic are
  structurally authored but not semantically validated. Calling
  `typecheck(lang, ast)` on a real program returns errors (graceful
  degradation — the adapter returns `LangError::NotImplemented` or
  the model returns diagnostics).
- **Full typechecking is not working end-to-end.** No real program has
  been successfully typechecked through the Handl models.
- **`hnl typecheck` still uses the Rust bootstrap path**
  (`check_module_with_prelude`), not the Handl models. The dogfood gate
  is unaffected.
- **stdlib `array.is_empty.hnl` has a parse error.** This does not
  block the adapter (the model loads without it), but it is a known
  gap to fix.
- **NBE test suite not ported.** The 11,364 lines of NBE tests have
  not been ported to Handl-level parity tests.
- **Rust typecheck crates are NOT deleted.** They remain in the
  workspace and are still the production typechecker.

### Commit history (this session)

- `152932ad` — feat: author Handl typecheck models and adapter for
  ADR-0105 (Phase 1 + Phase 2: models + adapter crate).
- `a2b5b11f` — feat: wire HandlTypecheckEngine into kit-canonical with
  parity tests (Phase 3 start: wiring + 23 tests).
- `d017b4b7` — feat: wire stdlib deps in adapter and extend
  type_model.hnl with HoTT ops (stdlib fix + 26 new judgment ops).

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
