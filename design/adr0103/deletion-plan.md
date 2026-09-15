# Plan: Delete the Rust NBE typechecker after `hnl typecheck --handl` is production-ready

This document is a branch-ready execution plan. It does **not** delete any
crates now; it records the dependency map, the registration sites, the test
coverage that must move, the `TypecheckEngine` trait decision, the exact
wiring swaps, the `Cargo.toml` surgery, and the ADR text to add.

**Precondition before starting the deletion:** `hnl typecheck --handl` passes
the dogfood gate over `handl-packages/` and `examples/` with the same result
as the current NBE/bootstrap path, i.e. `project/design/adr0103/typecheck_model.hnl`
and `scope_model.hnl` are fully exercised.

**Baseline:** `cargo check --workspace` passed before this plan was written.
No code files were modified in this pass.

---

## 1. Crates to delete and what stays

### Delete in Phase 1

- `crates/plugin-engine-typecheck-nbe` (the NBE implementation).
- Its `src/tests.rs` and `tests/` integration tests go with the crate.

### Keep / repurpose in Phase 1

- `crates/plugin-engine-typecheck-handl` — the new default engine.
- `crates/plugin-engine-typecheck` — keep for now because it still owns:
  - the `TypecheckEngine` trait and the `TypecheckRequest`/`TypecheckResult`
    data types (used by `handl-runtime`, `kit-canonical`, `plugin-lib-lang-typecheck`,
    `plugin-ext-lint`, `host-cli`);
  - `bootstrap.rs` and `check_module_with_prelude`, which powers `hnl lint`
    (`unbound-wire` pass) and the current `hnl typecheck` diagnostic path.

### Delete in Phase 2 (after scope/totality Handl models replace bootstrap)

- `crates/plugin-engine-typecheck`, **only after**:
  1. `bootstrap.rs` is replaced by `scope_model.hnl` / `totality_model.hnl`;
  2. `check_module` and `check_module_with_prelude` are removed or moved;
  3. the `TypecheckEngine` trait is relocated (see §4).

---

## 2. Direct and transitive dependency map

### `handl-plugin-engine-typecheck-nbe`

**Direct dependencies** (from `crates/plugin-engine-typecheck-nbe/Cargo.toml`):

- `handl-language`
- `handl-platform`
- `handl-plugin-engine-exec`
- `handl-plugin-engine-typecheck`
- `plugin-engine`
- `handl-plugin-lib-tag`
- `serde_json`

**Dev dependencies:**

- `handl-kit-data-canonical`
- `handl-plugin-engine-exec-tree-walk`
- `handl-plugin-engine-lex-pattern`
- `handl-plugin-engine-parse`
- `handl-plugin-engine-parse-pratt`
- `handl-plugin-module`

**Transitive workspace dependencies reached via the above:**

- `handl-plugin-engine-lex`, `handl-plugin-engine-lex-pattern`
- `handl-plugin-engine-parse`, `handl-plugin-engine-parse-pratt`
- `handl-plugin-engine-exec-tree-walk`
- `handl-plugin-engine-transform`, `handl-plugin-engine-transform-rewrite`
- `handl-plugin-engine-lint`
- `handl-plugin-lib-handl`, `handl-plugin-lib-stdlib`, `handl-plugin-module`
- `handl-kit-data`, `handl-kit-data-canonical`, `handl-emitters`, `handl-runtime`
- `handl-value`, `handl-test-utils` (via test setup)

All of these continue to exist when the crate is deleted because they are used
by other crates.

### `handl-plugin-engine-typecheck`

**Direct dependencies** (from `crates/plugin-engine-typecheck/Cargo.toml`):

- `handl-language`
- `handl-platform`
- `handl-plugin-ext-diag`
- `handl-plugin-engine-lex-pattern`
- `handl-plugin-engine-parse`
- `handl-plugin-engine-parse-pratt`
- `serde_json`

**Transitive dependencies:** the `handl-*` lexer/parser/emitters plus
`serde`/`thiserror`/`toml`.

---

## 3. Registration and reverse-dependency sites

| Consumer                    | File                                                       | Uses                                                                          | Line(s) / Notes             |
| --------------------------- | ---------------------------------------------------------- | ----------------------------------------------------------------------------- | --------------------------- |
| `kit-canonical`             | `crates/kit-canonical/src/lib.rs`                          | `use handl_plugin_engine_typecheck_nbe::NbeTypecheckEngine;`                  | 34                          |
|                             |                                                            | `use handl_plugin_engine_typecheck_handl::HandlTypecheckEngine;`              | 33                          |
|                             |                                                            | `use handl_plugin_engine_typecheck::TypecheckEngine;`                         | 32                          |
|                             |                                                            | `registry.register_typecheck("nbe", ...)`                                     | 1403–1408                   |
|                             |                                                            | `registry.register_typecheck("handl", ...)`                                   | 1409–1414                   |
|                             |                                                            | `registry.register_typecheck("nbe", ...)` in `build_kit`                      | 1907–1912                   |
|                             |                                                            | `Kit::typecheck` → `self.runtime.typecheck(&ast)`                             | 1748–1752                   |
|                             |                                                            | `Kit::typecheck_handl`                                                        | 1769–1796                   |
| `handl-runtime`             | `crates/handl-runtime/src/lib.rs`                          | `pub use handl_plugin_engine_typecheck::TypecheckResult;`                     | 81                          |
|                             |                                                            | `use handl_plugin_engine_typecheck::{TypecheckEngine, TypecheckRequest};`     | 82                          |
|                             |                                                            | `EngineRegistry::typecheck` map                                               | 228                         |
|                             |                                                            | `RuntimeConfig::typecheck` field                                              | 357                         |
|                             |                                                            | `Runtime::typecheck` method                                                   | 835–842                     |
|                             |                                                            | `Runtime::from_registry_with_resolver` typecheck engine resolution            | 1147–1167                   |
| `host-cli`                  | `crates/host-cli/src/main.rs`                              | `handl-plugin-engine-typecheck` dep / `check_module_with_prelude` call        | 535–545                     |
|                             |                                                            | `typecheck_command` `--handl` flag                                            | 403–416                     |
|                             |                                                            | `typecheck_one` `use_handl` branch                                            | 510–526                     |
| `plugin-ext-lint`           | `crates/plugin-ext-lint/src/lib.rs`                        | `handl_plugin_engine_typecheck::check_module_with_prelude`                    | 94, 133                     |
| `plugin-lib-lang-typecheck` | `crates/plugin-lib-lang-typecheck/src/lib.rs`              | `TypecheckResult` re-export not used directly; calls `handle.typecheck(&ast)` | 138–141                     |
| `handl-kit-data-canonical`  | `crates/handl-kit-data-canonical/data/kits/canonical.yaml` | `runtime.typecheck: nbe`                                                      | 13                          |
|                             |                                                            | `crates/handl-kit-data-canonical/src/lib.rs`                                  | doc line 333; test line 792 |

---

## 4. Tests that depend on NBE behavior

### In `crates/plugin-engine-typecheck-nbe`

1. `src/tests.rs` — ~11,000 lines of NBE-specific unit tests.
   - These exercise `NbeTypecheckEngine`, `TypeModel`, registry ids, `with_pure`,
     judgment ops, dependent types, FFI marshalling, etc.
   - **Disposition:** delete with the crate. They are not general typecheck
     contracts; the equivalent contracts should be expressed against the Handl
     engine or the canonical dogfood corpus after parity is proven.
2. `tests/parity.rs` — cross-engine parity vs. the legacy `plugin-typecheck-nbe` IR.
   - **Disposition:** delete. There is no legacy engine to compare with.
3. `tests/grade_type_dispatch.rs` — `TypeModel` / grade dispatch.
   - **Disposition:** delete. The concept is specific to the NBE engine.
4. `tests/with_handle_type_e2e.rs` — handle/FFI typechecking end-to-end.
   - **Disposition:** if the Handl models already cover these cases, migrate
     to `crates/plugin-engine-typecheck-handl/tests`; otherwise add equivalent
     `handl_typecheck_parity.rs` cases before deleting.

### In `crates/plugin-engine-typecheck-nbe` but also used by the new engine

The `tests/` that compare NBE to itself or to the old IR are gone. All
**canonical behavior** must be captured in `crates/plugin-engine-typecheck-handl/tests`
and the `kit-canonical` / `handl-runtime` test suites.

### In `crates/kit-canonical`

Unit tests inside `src/lib.rs` (around lines 2583–3135) call `kit.typecheck(...)`
and assert the synthesized type for canonical surface forms.

**Disposition:** these must be updated to:

- expect the Handl engine output (they are already run through
  `Runtime::typecheck` with whatever engine is selected);
- after the default is changed to `handl`, any test that previously relied on
  NBE-specific implementation details (e.g. exact wording of diagnostics) must
  be adjusted to the Handl model output.

### In `crates/handl-kit-data-canonical/tests`

- `tests/kit_assembly.rs:68` asserts `kit.runtime_config().typecheck == "nbe"`.
  - **Disposition:** change to `"handl"`.
- `tests/full_language_parity.rs:333` and `348` comments reference the `nbe` engine.
  - **Disposition:** update comments to reference the Handl engine.
- `tests/proc_notation.rs:47` comment references `nbe` engine impl.
  - **Disposition:** update comment.
- `tests/graded_theory.rs`, `tests/with_handle.rs`, `tests/proc_notation.rs`
  clear `config.typecheck` to test lex/parse/eval without a typechecker.
  - **Disposition:** keep as-is; they intentionally do not depend on NBE.

### In `crates/kit-canonical/tests/fp_integration.rs`

- `kit.runtime().typecheck(&ast)` is called at line 866.
  - **Disposition:** keep the call; the runtime will resolve the `handl` engine.
    Update the comment / assertion expectations if the Handl model produces
    different error diagnostics.

### In `crates/plugin-engine-typecheck-handl/tests`

- `tests/handl_typecheck_parity.rs` and `tests/integration.rs` already exercise
  the `HandlTypecheckEngine`.
  - **Disposition:** expand these with any fixture from the NBE parity tests
    that is worth preserving; delete NBE engine.

---

## 5. `TypecheckEngine` trait: keep or delete?

**Decision: keep the abstraction, but move it out of `plugin-engine-typecheck`
in Phase 2.**

Reasons to keep:

- `handl-runtime` dispatches on `Box<dyn TypecheckEngine>` (ADR-0054).
- `plugin-lib-lang-typecheck` exposes `typecheck.typecheck(lang, ast)` and
  builds a `RuntimeHandle` that calls `handle.typecheck(&ast)`.
- The `HandlTypecheckEngine` is only one possible engine; the trait keeps the
  door open for future specialized engines (e.g. an SMT backend).

Phase 1:

- Leave `TypecheckEngine` in `crates/plugin-engine-typecheck/src/lib.rs`.
- Leave `bootstrap.rs` in place because `plugin-ext-lint` and the current
  `hnl typecheck` diagnostic path still need `check_module_with_prelude`.

Phase 2 (after `scope_model.hnl` replaces `bootstrap.rs`):

1. Move `TypecheckEngine`, `Typecheck`, `TypecheckRequest`,
   `TypecheckResult`, `TypecheckDiagnostic` from `plugin-engine-typecheck`
   into `crates/plugin-engine/src/lib.rs` (or `crates/handl-runtime/src/lib.rs`
   if that breaks fewer imports).
2. Update `use handl_plugin_engine_typecheck::*` statements in:
   - `crates/handl-runtime/src/lib.rs`
   - `crates/kit-canonical/src/lib.rs`
   - `crates/plugin-engine-typecheck-handl/src/lib.rs`
   - `crates/plugin-lib-lang-typecheck/src/lib.rs`
   - `crates/host-cli/src/main.rs`
3. Delete `crates/plugin-engine-typecheck` once `check_module` is no longer
   referenced.

---

## 6. Wiring changes (branch-ready diff checklist)

### 6.1 Canonical kit default engine

File: `crates/handl-kit-data-canonical/data/kits/canonical.yaml`

```yaml
runtime:
  lex: data
  parse: pratt
  eval: tree-walk
  typecheck: handl # was: nbe
```

This flips `Runtime` typecheck resolution to Handl for every `Kit` built from
canonical data.

### 6.2 `kit-canonical` registration

File: `crates/kit-canonical/src/lib.rs`

1. Remove the NBE import:

```rust
// DELETE line 34:
// use handl_plugin_engine_typecheck_nbe::NbeTypecheckEngine;
```

2. Remove the two `nbe` engine registrations (lines 1403–1408 and 1907–1912).
   Keep only the `handl` registration:

```rust
registry.register_typecheck(
    "handl",
    Box::new(HandlTypecheckEngine::new(Box::new(
        handl_plugin_engine_exec_tree_walk::TreeWalkEngine::new(),
    ))),
);
```

3. Add a `Kit::typecheck_report` method and make `Kit::typecheck` use it:

```rust
impl Kit {
    /// Typecheck `input` and return the full `TypecheckResult`.
    ///
    /// Always uses the Handl-authored engine (ADR-0105).
    pub fn typecheck_report(&self, input: &str) -> Result<TypecheckResult, LangError> {
        let ast = self.parse(input)?;
        let mut cell = self.handl_typecheck_engine.borrow_mut();
        if cell.is_none() {
            *cell = Some(HandlTypecheckEngine::new(Box::new(
                handl_plugin_engine_exec_tree_walk::TreeWalkEngine::new(),
            )));
        }
        let engine = self.handl_typecheck_engine.borrow();
        let engine = engine.as_ref().ok_or_else(|| LangError::engine("handl engine missing"))?;
        let req = handl_plugin_engine_typecheck::TypecheckRequest::new(ast);
        engine.typecheck(self.runtime.lang(), &req)
    }

    /// Typecheck `input`, returning the synthesized type.
    ///
    /// This is the convenience `Value`-returning wrapper; for diagnostics,
    /// use [`Kit::typecheck_report`].
    pub fn typecheck(&self, input: &str) -> Result<Value, LangError> {
        let result = self.typecheck_report(input)?;
        if result.diagnostics.iter().any(|d| d.severity == "error") {
            let messages = result
                .diagnostics
                .iter()
                .map(|d| d.message.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(LangError::engine(format!("typecheck errors: {messages}")));
        }
        Ok(result.typ)
    }

    // `typecheck_handl` can be removed or kept as an alias.  If kept:
    // pub fn typecheck_handl(&self, input: &str) -> Result<Value, LangError> { self.typecheck(input) }
}
```

### 6.3 `host-cli` default

File: `crates/host-cli/src/main.rs`

Change `typecheck_command` so the default is the Handl path and the `--handl`
flag becomes a no-op (or is removed).

```rust
fn typecheck_command(args: &[String]) -> i32 {
    let mut paths = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--" => {}
            "--handl" => { /* kept for backward compatibility; default now */ }
            s if s.starts_with("--") => {
                eprintln!("typecheck: unknown flag `{s}`");
                return 2;
            }
            s => paths.push(s.to_string()),
        }
    }
    // ... existing setup ...
    let code = typecheck_one(&kit, lang, &src, path, &mut had_failure);
    // ...
}
```

Then simplify `typecheck_one` to call `Kit::typecheck_report` and render the
returned diagnostics:

```rust
fn typecheck_one(
    kit: &Kit,
    _lang: &Language,
    src: &str,
    path: &str,
    had_failure: &mut bool,
) -> i32 {
    match detect_dialect(src) {
        Dialect::Contract => /* unchanged */,
        Dialect::Arrow => 0,
        Dialect::Canonical => {}
    }

    let result = match kit.typecheck_report(src) {
        Ok(r) => r,
        Err(err) => {
            eprintln!("{path}: {err}");
            if matches!(err, LangError::LexError { .. }) {
                return 2;
            }
            *had_failure = true;
            return 0;
        }
    };

    // Render all diagnostics (warnings, notes, errors) exactly like
    // the current bootstrap path.
    let rendered = render_diagnostics(
        &result.diagnostics.iter().map(|d| handl_plugin_ext_diag::Diagnostic {
            severity: d.severity.as_str().into(),
            message: d.message.clone(),
            span: d.span.clone(),
            // ... map other fields as needed
        }).collect::<Vec<_>>(),
        path,
    );
    if !rendered.is_empty() {
        println!("{rendered}");
    }

    if result.diagnostics.iter().any(|d| d.severity == "error") {
        *had_failure = true;
    } else {
        println!("{path}: ok: {}", result.typ);
    }
    0
}
```

> Note: `TypecheckDiagnostic` does not yet carry the same fields as
> `handl_plugin_ext_diag::Diagnostic`. Before deleting `plugin-engine-typecheck`,
> ensure the mapping is implemented (add `span`, `severity` enum conversion, etc.)
> or keep `check_module_with_prelude` for the CLI.

### 6.4 `handl-runtime`

File: `crates/handl-runtime/src/lib.rs`

No code change required if the canonical manifest is flipped to `handl`.
`Runtime::typecheck` already resolves `config.typecheck` against the registry.

If any unit tests in `crates/handl-runtime/tests/` explicitly create a
`RuntimeConfig` with `typecheck` set to `"nbe"`, update them to `"handl"`.

### 6.5 `plugin-lib-lang-typecheck`

File: `crates/plugin-lib-lang-typecheck/src/lib.rs`

The `typecheck.typecheck(lang, ast)` meta function builds a `RuntimeHandle`
from the injected `RuntimeFactory`. After the canonical manifest and
`kit-canonical` registrations are updated, that sub-runtime will resolve the
`handl` engine automatically. No code change required.

### 6.6 `plugin-ext-lint`

File: `crates/plugin-ext-lint/src/lib.rs`

Phase 1: leave `handl-plugin-engine-typecheck` dependency and
`check_module_with_prelude` calls untouched.

Phase 2: when `scope_model.hnl` is ready, replace the Rust unbound-wire scope
pass with a call to the Handl scope model (either through a new
`Kit::check_scope` or by reusing `HandlTypecheckEngine` with a scope-only
request).

---

## 7. `Cargo.toml` workspace surgery

### Phase 1 — remove only the NBE crate

1. `Cargo.toml` (workspace root)
   - Remove `crates/plugin-engine-typecheck-nbe` from `[workspace].members`
     (line 102).
   - Remove `handl-plugin-engine-typecheck-nbe = { path = "crates/plugin-engine-typecheck-nbe" }`
     from `[workspace.dependencies]` (line 241–242).

2. `crates/kit-canonical/Cargo.toml`
   - Remove `handl-plugin-engine-typecheck-nbe = { workspace = true }` (line 18).

3. Delete the directory `crates/plugin-engine-typecheck-nbe`.

4. Run `cargo check --workspace` to regenerate `Cargo.lock`.

### Phase 2 — remove `plugin-engine-typecheck` (after bootstrap replacement)

1. `Cargo.toml` (workspace root)
   - Remove `crates/plugin-engine-typecheck` from `[workspace].members`
     (line 101).
   - Remove `handl-plugin-engine-typecheck = { path = "crates/plugin-engine-typecheck" }`
     from `[workspace.dependencies]` (line 240).

2. Remove the `handl-plugin-engine-typecheck` dependency from:
   - `crates/handl-runtime/Cargo.toml` (line 12)
   - `crates/kit-canonical/Cargo.toml` (line 17)
   - `crates/host-cli/Cargo.toml` (line 24)
   - `crates/plugin-ext-lint/Cargo.toml` (line 11)
   - `crates/plugin-lib-lang-typecheck/Cargo.toml` (line 15)
   - `crates/plugin-engine-typecheck-handl/Cargo.toml` (line 11) —
     this will instead depend on `plugin-engine` (or `handl-runtime`) where the
     trait was moved.

3. Delete the directory `crates/plugin-engine-typecheck`.

4. Run `cargo check --workspace`.

---

## 8. ADR text to add

### ADR-0105 `project/adrs/0105-typechecker-in-handl.md`

Append the following section before the "Status" or as a new subsection under
"Implementation".

```markdown
### Phase 6 — Deletion of the Rust NBE typechecker

Once `hnl typecheck --handl` passes the full dogfood gate:

1. Delete `crates/plugin-engine-typecheck-nbe` and all its tests.
2. Change the canonical kit manifest (`canonical.yaml`) to select the
   `handl` typecheck engine by default.
3. Change `kit-canonical` to register only the `handl` engine and to expose
   `Kit::typecheck_report` for full diagnostic rendering.
4. Change `host-cli` `hnl typecheck` to default to the Handl path.
5. Keep `crates/plugin-engine-typecheck` until `scope_model.hnl` and
   `totality_model.hnl` fully replace `bootstrap.rs`; then move the
   `TypecheckEngine` trait into `plugin-engine` and delete the crate.

After Phase 6, typechecking is entirely Handl-authored: `typecheck_model.hnl`
drives the AST traversal and unification, `scope_model.hnl` handles the module
surface checks, and `totality_model.hnl` handles termination. The only Rust
seam is the `TypecheckEngine` trait and the `HandlTypecheckEngine` adapter.
```

### ADR-0103 `project/adrs/0103-kernel-is-evaluator-not-calculus.md`

Update the Phase 5 / implementation summary paragraph about the NBE typechecker
(near line 37 / 48–55) to:

```markdown
The NBE typechecker crate (`plugin-engine-typecheck-nbe`) has been deleted
(ADR-0105 Phase 6). Typechecking is now implemented by the Handl-authored
models in `project/design/adr0103/` (`typecheck_model.hnl`, `scope_model.hnl`,
`totality_model.hnl`). A thin `TypecheckEngine` trait remains as the Rust
boundary so `handl-runtime` can resolve and dispatch the engine by name; the
concrete logic lives entirely in the Handl models.
```

---

## 9. Verification checklist

Before the branch is merged:

- [ ] `cargo check --workspace` passes.
- [ ] `cargo test --workspace` passes (NBE tests are gone; Handl parity tests
      cover the same canonical surface).
- [ ] `hnl typecheck` with no `--handl` flag produces the same exit codes and
      diagnostics as the current NBE/bootstrap path over the dogfood corpus.
- [ ] `hnl typecheck --handl` is now the default and the flag is either a no-op
      or removed.
- [ ] `hnl lint` still works (because `plugin-engine-typecheck` / `bootstrap.rs`
      is still present in Phase 1).
- [ ] The deleted crates are removed from `Cargo.toml` members,
      `[workspace.dependencies]`, and any consumer `Cargo.toml`.
- [ ] `Cargo.lock` is regenerated and committed.

---

## 10. What was _not_ done in this pass

- No crates were deleted.
- No `Cargo.toml` or `.rs` files were modified (except for the creation of this
  plan document).
- No commit was made.
- `cargo check --workspace` was run only to establish a clean baseline; it
  passed.
