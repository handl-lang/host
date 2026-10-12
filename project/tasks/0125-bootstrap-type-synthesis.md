# Task: the Rust bootstrap type synthesiser (ADR-0125)

Status: **in progress — recovery first.**

## The surprise that shapes this plan

The ADR was written assuming the judgment ops had to be *written*. They do not:
the deleted `plugin-engine-typecheck-nbe` crate is in git history and **already
implements the design**.

```
crates/plugin-engine-typecheck-nbe/src/lib.rs        5488 lines   <- the engine
crates/plugin-engine-typecheck-nbe/src/tests.rs     11410 lines
crates/plugin-engine-typecheck-nbe/tests/parity.rs    493 lines   <- parity harness
```

Deleted in `a5ff866f` ("feat(host-cli): default hnl typecheck to the Handl
engine") — i.e. the Rust engine was retired precisely *because* the Handl model
became the default. This task is that decision, reversed for the bootstrap path.

It is already shaped like the ADR's decision:

```rust
// line 275ff
let rule = lang.types.get_by_identity(&node.kind).ok_or_else(|| ...)?;
let typ = self.eval_type_scheme(lang, ast, node_ref, &rule.synth, ctx, None)?;
ctx.node_types.insert(node_ref.index(), typ.clone());
```

and it imports only crates that still exist: `handl_language`,
`handl_plugin_engine_exec::{EvalEngine, EvalRequest}`,
`handl_plugin_engine_typecheck::{…}`, `handl_plugin_lib_tag::TypeRegistry`,
`plugin_engine::TermData`.

### Op coverage

Measured against today's `types.yaml` (40 distinct ops, 469 uses):

- **recoverable from the NBE crate: 32 of 40** — including the entire core
  `construct` (110), `result` (78), `bind` (65), `synth_child` (64),
  `unify` (47), `check_child` (35), i.e. 85% of all uses.
- **missing (newer than the crate): 8** — `construct_family`,
  `synth_axis_flow`, `synth_data`, `synth_dim_arrow`, `synth_flow`, `synth_j`,
  `synth_transport` (the ADR-0119/0120 dimension/grade and axis-flow work),
  plus `synth_child`'s alias handling to re-verify.

## Plan

1. **Recover** the crate from `a5ff866f^`, re-register it in the workspace.
2. **Compile it** and turn the adaptation gap into a concrete error list —
   that list, not a guess, is the real scope of "adapt to the current API".
3. **Fix the compile errors** (API drift since deletion).
4. **Implement the 8 missing ops.**
5. **Parity-test** against the Handl model, reusing the recovered
   `tests/parity.rs` harness. Restore as much of `tests.rs` as still applies.
6. **Flip the default**: register the bootstrap engine as the runtime typecheck
   engine; keep the model opt-in via `Kit::typecheck_handl*`.
7. **Tighten `MAX_MODEL_NODES`** to the measured cliff, and give
   `Kit::typecheck`'s fallback a real type instead of `Value::Null`.
8. **Fix `canonical.yaml`** (`runtime.typecheck: nbe` names a crate that no
   longer exists).
9. Full suite + record before/after wall clock.

## Progress log

### Step 1-2 — recovered and compiling ✅

The crate is back (`f688a53d`'s parent, `0a5521bd`) and `cargo check` is clean
after **three** surgical edits, all in effect-row handling:

| drift | fix |
| --- | --- |
| `handl_language::{EffectRow, infer_effect_row}` gone (ADR-0119) | drop the effect-row enrichment from the `Handle` arm |
| `handl_language::eliminate_and_check` gone | `Term::eliminate_handlers()` (`plugin-engine/src/term.rs`) |
| `effect_row_to_type_value` used the removed `EffectRow` | delete it |

Dropping the effect row is parity-safe, not a compromise: `types.yaml`'s
`handle` rule is a bare `synth_child(0)` (no effect ops) and the Handl model
does no effect inference at all.

### Step 3 — tests compile ✅

`cargo check --tests` is clean (`f688a53d`). Three mechanical categories:

- 6 `ChildRoleSpec` literals missing `optional`.
- 4 tests using `Term::CellId`/`Refine`/`JElim`, variants retired from the
  calculus — removed rather than ported (the lib still handles those scheme
  *names*, but as `TermData`, not `Term` variants).
- 7 `Term::Arr`/`Term::Op` literals missing `dim` (ADR-0119); each got
  `Const(1)`, matching the crate-internal `default_dim_value()`.

### Step 4 — the suite runs: **282 / 299 pass**

The 17 failures fall into three groups, in priority order:

1. **13 × `parity_*` — all one root cause: `unbound variable 'true'`.** The
   failures are exactly the boolean/logical cases (`parity_bool_true/false`,
   `parity_if_*`, `parity_and/or`, `parity_unary_not`, `parity_ternary_*`,
   `parity_type_mismatch_and_errors`), while arithmetic/logic-free parity
   tests pass (`parity_eq`, `parity_div`, `parity_gt`, `parity_gte`, …).
   **This is a wiring gap, not an algorithm bug.** `parity.rs` assembles a
   bare `Language` (`handl_kit_data_canonical::assemble`) and constructs
   `TypecheckContext::new("handl.canonical", HashMap::new())` with an empty
   globals map — but `true`/`false` are **module values**, declared in
   `plugin-lib-stdlib`'s package table (`bool` → `values: &["true", "false"]`),
   which only a *kit* seeds, not a `Language`. Fix: seed them in the harness
   (or have the engine seed builtin type-level constants), then re-check.
2. **2 × `b2_parametric_predicate_call_{proven,disproved}`** — the engine
   returns `Blocked` where the test expects `Proven`/`Disproved` (a
   refinement/predicate verdict, `predicate_eval.rs`). Not yet investigated;
   could be genuinely superseded behaviour or another seeding gap.
3. **2 × `effect_row_handle_attaches_effects_to_function_type`,
   `effect_row_nested_handles_union_ops`** — **expected**: they assert the
   effect-row behaviour deliberately dropped above. Delete them with the
   feature, or reinstate effect rows against the current capability API
   (`handl-language/src/capabilities.rs`).

Note the ADR-0125 "parity" is against the **canonical language's own rules**
(per `parity.rs`'s docstring), not against the Handl model. A true
engine-vs-model parity test is still to be written — that is the gate the ADR
requires before flipping the default.

## Guardrails

- **Do not flip the default until parity is green**, or `hnl typecheck` output
  changes silently.
- **Do not delete the model** — it is the parity oracle.
- Do this on a branch-free, commit-per-step basis so a bad step is a `git
  revert` away (`main` is the working branch here; each step must build).
