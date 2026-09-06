# ADR-0032 Item 6: Proof Erasure at Runtime

## Goal

Wire proof-term erasure into the `Runtime::run_checked` pipeline so that the
typechecker can tell the runtime which AST nodes are proofs, and the runtime
replaces those nodes with a cheap `erased`/`null` node before evaluation.
This removes the runtime cost of carrying proof terms while keeping the
static type-safety guarantees intact.

## Status

in progress

## Subtasks

### 1. Add the `erased` AST node and eval scheme

Status: **done**

- Add an `ast.erased` leaf shape to
  `crates/handl-kit-data-canonical/data/ast.yaml` (no children, empty/null
  payload).
- Add an `eval.erased` scheme to
  `crates/handl-kit-data-canonical/data/eval.yaml` that evaluates to `null`
  (e.g. `!Arr op: const` with no literal, so the existing tree-walk `const`
  native op returns `Value::Null`).
- Add a unit or integration test verifying that an AST containing an `erased`
  node evaluates to `Value::Null`.
  - Candidate locations:
    `crates/plugin-engine-exec-tree-walk/src/lib.rs` tests (native `const` tests
    already exist around line 3942),
    `crates/handl-runtime/src/lib.rs` tests, or a new
    `crates/handl-runtime/tests/proof_erasure.rs`.

Files expected to change:
- `crates/handl-kit-data-canonical/data/ast.yaml`
- `crates/handl-kit-data-canonical/data/eval.yaml`
- `crates/handl-runtime/src/lib.rs` or `crates/handl-runtime/tests/proof_erasure.rs`
  (or `crates/plugin-engine-exec-tree-walk/src/lib.rs`)

Verification:
- `cargo test -p handl-plugin-engine-exec-tree-walk`
- `cargo test -p handl-runtime`

Result (completed):

- `ast.erased` shape appended to `data/ast.yaml` (no children,
  `payload_schema: {type: "null"}`).
- `eval.erased` scheme appended to `data/eval.yaml` (`!Arr op: const`,
  `args: []`, `source: Native`, no `lit` → returns `Value::Null`).
- New integration test `crates/handl-runtime/tests/erased_node.rs`:
  `erased_node_evaluates_to_null` builds the canonical `Kit`, pushes an
  `erased` node with `null` payload, and asserts eval yields `null`.
- Verification run:
  - `cargo test -p handl-runtime --test erased_node` — 1 passed.
  - `cargo test -p handl-runtime` — all targets pass (120+ tests, 0 failed).
  - `cargo test -p handl-plugin-engine-exec-tree-walk` — all pass (122 tests).
  - `cargo test -p handl-kit-canonical` — 69 passed, **2 pre-existing
    failures** (`hello_greet_returns_canonical_greeting`,
    `hello_greet_bridge_escapes_special_chars`) that also fail with the new
    YAML entries reverted — unrelated dirty-tree state
    (`check_json_value: expected type is not a tagged type value` at
    `crates/kit-canonical/src/lib.rs:1031`).

### 2. Expose a per-node type map in `TypecheckResult`

Status: **done**

- Extend `TypecheckResult` in `crates/plugin-engine-typecheck/src/lib.rs` with a
  new field, e.g. `pub node_types: HashMap<usize, Value>` (keyed by
  `AstRef::index()`). Keep it backward-compatible by defaulting to an empty map.
- In `crates/plugin-engine-typecheck-nbe/src/lib.rs`:
  - Add `node_types: HashMap<usize, Value>` to `TypecheckContext`
    (around line 3991).
  - Record the synthesized/checked `Value` for every visited `AstRef` in
    `synth_node` (line 261) and `check_node` (line 285).
  - Populate `node_types` in `NbeTypecheckEngine::typecheck` (line 3939).
- Update the existing `TypecheckResult` constructors in test stubs:
  `crates/handl-runtime/src/lib.rs` (lines 1496 and 2229) and
  `.ref-crates/plugin-lib-lang-typecheck/src/tests.rs` (line 68).
- Add a test in `crates/plugin-engine-typecheck-nbe/src/tests.rs` that
  typechecks a small expression and asserts `result.node_types` contains
  entries for the root and at least one child.

Files expected to change:
- `crates/plugin-engine-typecheck/src/lib.rs`
- `crates/plugin-engine-typecheck-nbe/src/lib.rs`
- `crates/plugin-engine-typecheck-nbe/src/tests.rs`
- `crates/handl-runtime/src/lib.rs` (test stubs)
- `.ref-crates/plugin-lib-lang-typecheck/src/tests.rs`

Verification:
- `cargo test -p handl-plugin-engine-typecheck-nbe`
- `cargo test -p handl-runtime`
- `cargo test -p handl-kit-canonical`

Result (completed):

- `TypecheckResult` gained `pub node_types: HashMap<usize, Value>`
  (`crates/plugin-engine-typecheck/src/lib.rs`), keyed by `AstRef::index()`.
- `TypecheckContext` gained `node_types` (initialized in
  `TypecheckContext::new`); `synth_node` records the synthesized type and
  `check_node` records `expected` on the `check_scheme` branch and the
  synthesized `ty` on the synth-then-unify branch.
- `NbeTypecheckEngine::typecheck` returns `node_types: ctx.node_types`.
- Stub constructors updated: `crates/handl-runtime/src/lib.rs` (two sites)
  and `.ref-crates/plugin-lib-lang-typecheck/src/tests.rs` — all use
  `node_types: Default::default()`.
- New test `node_types_populated_after_typecheck` in
  `crates/plugin-engine-typecheck-nbe/src/tests.rs`: a `wrap{children:[0]}`
  AST over a `lit` child; asserts `node_types[1]` (root) and
  `node_types[0]` (child) both equal `type.number`.
- Verification run:
  - `cargo test -p handl-plugin-engine-typecheck-nbe` — 251 + 46 tests, all
    pass (includes the new test).
  - `cargo test -p handl-runtime` — all targets pass (100 lib + integration
    tests, 0 failed).
  - `cargo test -p handl-plugin-engine-exec-tree-walk` — all pass.
  - `cargo test -p handl-kit-canonical` — 69 passed, **2 pre-existing
    failures** (`hello_greet_returns_canonical_greeting`,
    `hello_greet_bridge_escapes_special_chars`,
    `check_json_value: expected type is not a tagged type value`) — same
    failures reported in subtask 1, unrelated dirty-tree state.

### 3. Implement `erase_pass` and wire it into `Runtime::run_checked`

Status: **done**

- Created `crates/handl-runtime/src/erase_pass.rs`:
  - `apply_erase(lang, ast, node_types)` performs an in-place arena mutation.
  - Proof values are recognized by `__tag__` (`type.refl`, `type.is_evidence`,
    `type.empty`) and optional `__registry__ == "handl.canonical"`.
  - Matching nodes are rewritten to the `erased` node kind with a `null`
    payload. The `erased` kind is resolved from `lang.ast` by identity.
- Wired `erase_pass::apply_erase` into `Runtime::run_checked` after the
  `runtime_assert` pass and before `eval` (ADR-0032).
- Added `pub mod erase_pass;` in `crates/handl-runtime/src/lib.rs`.
- Added tests:
  - Unit tests in `erase_pass.rs`: proof erasure, non-proof preservation,
    missing type info, and missing `__registry__`.
  - Integration test `crates/handl-runtime/tests/proof_erasure.rs`:
    - `run_checked_erases_proof_typed_root` — a `value(42)` node whose
      typechecker reports `type.refl` evaluates to `null`.
    - `run_checked_preserves_computational_nodes` — a `value(42)` node
      whose type is `type.number` still evaluates to `42`.

Files changed:
- `crates/handl-runtime/src/erase_pass.rs` (new)
- `crates/handl-runtime/src/lib.rs`
- `crates/handl-runtime/tests/proof_erasure.rs` (new)

Verification (completed):
- `cargo test -p handl-runtime --test proof_erasure` — 2 passed.
- `cargo test -p handl-runtime` — 130+ tests passed (including new
  `erase_pass` unit tests and the `erased_node` / `proof_erasure`
  integration tests).
- `cargo test -p handl-plugin-engine-typecheck-nbe` — 251 + 46 passed.
- `cargo test -p handl-plugin-engine-exec-tree-walk` — 116 + integration
  passed.
- `cargo test -p handl-plugin-engine-exec-bytecode` — 71 + 4 passed.
- `cargo test -p handl-plugin-engine-exec-cranelift --lib` — 79 passed.
- `cargo test -p handl-kit-canonical` — 69 passed, 2 pre-existing failures
  (`hello_greet_returns_canonical_greeting`,
  `hello_greet_bridge_escapes_special_chars`); unrelated to erasure.
- `cargo test -p handl-kit-data-canonical` — lib tests passed; integration
  test `full_language_parity::language_has_eval_schemes_for_all_major_node_kinds`
  fails on missing eval scheme for `let` (pre-existing, unrelated to erasure).

### 4. End-to-end verification and ADR status

Status: **done**

- Ran the targeted crates (see subtask 3 verification) and confirmed that
  `data` desugaring, `match`, `runtime_assert`, and the new `erased` node work.
- Updated `project/adrs/0032-types-as-values-remaining.md`:
  - Marked item 6 (Proof erasure at runtime) as **completed**.
  - Added a note that built-in equality/membership proof erasure is in place
    and user-defined `data` proof-constructor erasure remains a follow-up.
- Left the optional `data` proof-constructor erasure in
  `crates/handl-runtime/src/desugar_pass.rs` as a future step; it requires
  surface syntax for marking constructors/types as `erased`/`proof`.

Files changed:
- `project/adrs/0032-types-as-values-remaining.md`
- `project/tasks/adr0032-proof-erasure.md`

Verification (completed):
- Same targeted crate runs as subtask 3.
- `data` desugaring tests (`indexed_family_data`, `indexed_family_match`,
  `indexed_family_fin`) and `runtime_assert` tests remain green.

## First subtask to implement next

All four subtasks are complete. The remaining follow-up is user-defined
`data` proof-constructor erasure (marking a constructor or whole `data`
declaration as `erased`/`proof` in surface syntax and dropping erased fields
from generated constructors/recursors).

## Related files

- `project/adrs/0032-types-as-values-remaining.md`
- `project/adrs/0082-inductive-types.md` (§5 — erasure deferred to this item)
- `project/adrs/0068-predicates-as-types.md` (Negative — no erasure yet)
- `crates/handl-runtime/src/lib.rs`
  - `Runtime::typecheck` at line 827
  - `Runtime::run_checked` at line 867
- `crates/handl-runtime/src/desugar_pass.rs`
  - `data` desugaring around line 1553
- `crates/handl-runtime/src/runtime_assert_pass.rs`
  - template for in-place AST mutation
- `crates/handl-kit-data-canonical/data/ast.yaml`
- `crates/handl-kit-data-canonical/data/eval.yaml`
- `crates/plugin-engine-typecheck/src/lib.rs`
  - `TypecheckResult` at line 83
- `crates/plugin-engine-typecheck-nbe/src/lib.rs`
  - `synth_node` at line 261
  - `check_node` at line 285
  - `NbeTypecheckEngine::typecheck` at line 3927
  - `TypecheckContext` at line 3991
  - equality/proof constructors (`construct_eq`, `check_refl`, `eq_reduce`)
    around line 2554
  - membership proof constructor (`check_membership`, `eval_membership`) around
    line 2753
- `crates/plugin-engine-typecheck-nbe/src/tests.rs`
  - `type_refl()` at line 7118
  - `type_is_evidence()` at line 6727
- `crates/handl-language/src/lib.rs`
  - `Value` / `TAG_VALUE_KEY` / `REGISTRY_VALUE_KEY`
- `crates/plugin-engine-exec-tree-walk/src/lib.rs`
  - `const` native op at line 1084

## Notes / open questions

- **What counts as a proof term today?**
  - Concrete singleton proof values are `type.refl` (equality proof) and
    `type.is_evidence` (membership proof), both minted by
    `TypeRegistry::construct` as tagged JSON objects with `__tag__` and
    `__registry__` keys.
  - `type.empty` is the uninhabited proposition; if a node ever synthesizes it,
    it is also safe to erase.
  - `type.eq` and `type.membership` are *proposition types* / type families,
    not proof values. Their inhabitants are `type.refl` and
    `type.is_evidence` respectively, so only those inhabitants should be
    erased.
  - `data` constructors are currently ordinary tagged records; there is no
    `proof` or `erased` marker, so user-defined proof types are not yet
    recognized by the erasure pass.
  - `Verdict::Proven` / `Verdict::Blocked` are compile-time outcomes of the
    predicate checker, not runtime values. They only control whether the
    `runtime_assert` pass runs.

- **Where should erasure run?**
  - The chosen design is an **AST pass between typecheck and eval**.
  - The typechecker records per-node types and returns them in
    `TypecheckResult::node_types`.
  - `Runtime::run_checked` calls `erase_pass::apply_erase` after the
    `runtime_assert` lowering and before `eval`.
  - Erasure is **not** done inside the typechecker (too engine-specific) and
    **not** as a runtime eval scheme (that would still build the proof value
    before discarding it, defeating the purpose).

- **Precondition / blocker:**
  `TypecheckResult` currently only returns the root type. Subtask 2
  (per-node type map) is the main blocker for the full erase pass.

- **Open question — ordering relative to `runtime_assert_pass`:**
  Proposed order is `typecheck` → `runtime_assert_pass` → `erase_pass` →
  `eval`. This keeps blocked runtime checks intact and only nullifies nodes
  whose type is already a singleton proof. Revisit if any erased constructor
  is needed by a `runtime_assert` predicate.

- **Open question — `data` proof constructors:**
  Built-in equality/membership proofs are covered by the `type.refl` /
  `type.is_evidence` tags. User-defined `data` proof types (e.g.
  `data Eq(a, b) = Refl`) will need an additional desugaring signal, such
  as an `erased` field on `DataConstructor`, before the erase pass can handle
  them. This is tracked as the optional follow-up in Subtask 4.
