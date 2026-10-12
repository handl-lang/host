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

## Guardrails

- **Do not flip the default until parity is green**, or `hnl typecheck` output
  changes silently.
- **Do not delete the model** — it is the parity oracle.
- Do this on a branch-free, commit-per-step basis so a bad step is a `git
  revert` away (`main` is the working branch here; each step must build).
