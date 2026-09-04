# Misc tasks — ADR-0068 / 0082 / 0083 follow-ups

Status snapshot taken at commit `8872ac9d` (FFI Phase 0 + ADR-0068 B1/B2
landing). Every item below was verified against the tree on 2026-09-04 and
is **not done**. Phases are ordered by dependency: earlier phases unblock
or make later phases testable. Within a phase, items are independent unless
noted.

Refs:
- ADR-0068 — predicates as types, partial-evaluator typechecker
- ADR-0082 — inductive types as desugared predicates
- ADR-0083 — FFI-native stdlib
- Engine-genericity rule (`.devin/rules/engine-genericity.md`): no hardcoded
  `kind ==` checks in engines; knowledge lives in `Language` data.

Legend: `[ ]` not started, `[~]` partial, `[x]` done. All `[ ]` below.

---

## Phase 0 — Critical `data`/recursor correctness bugs

These four bugs make the `data` desugar output dead code: it would either
produce wrong values, crash the `if` eval scheme, or hit an unbound
variable at runtime. Fix them before anything else in the `data`/recursor
track, because every later phase depends on the desugar actually working.

### 0.1 — Constructor `lit` doesn't substitute variable values  (Bug #1)

- **Where:** `crates/handl-runtime/src/desugar_pass.rs:800-816`
  (`desugar_data`, N-ary constructor branch).
- **Bug:** For `Succ(pred)`, the constructor body is built as a `lit` node
  whose payload is `{"tag": "Succ", "pred": "pred"}` — the *string*
  `"pred"`, not a reference to the `pred` parameter. At eval time the
  object field `pred` would hold the literal string, not the argument's
  value.
- **Root cause:** `obj.insert(arg_name.clone(), Value::from(arg_name.clone()))`
  inserts the arg *name* as a JSON string. The comment at line 806-809
  acknowledges this and says "use a `json` node" but ships the `lit` anyway.
- **Fix:** Build an object-construction node that references the arg
  variables. Options, in order of preference:
  1. A `json`/`object` node kind whose payload maps field names → child
     indices (each child is the arg `var` node). Add the kind to
     `data/ast.yaml` + `eval.yaml` (data-driven, per the engine-genericity
     rule).
  2. If no object-literal kind exists yet, desugar to a `block` of
     `assign`s into a fresh object literal — but this requires an
     object-allocation primitive. Prefer (1).
- **Test:** Add a unit test in `desugar_pass.rs` `tests` module that
  desugars `data Nat = Zero | Succ(Nat)` and asserts the `Succ` constructor
  body references the `pred` *var node*, not a string literal. Then add an
  eval test that constructs `Succ(Zero)` and checks the result is
  `{"tag": "Succ", "pred": {"tag": "Zero"}}`.
- **Files:** `crates/handl-runtime/src/desugar_pass.rs`,
  `crates/handl-kit-data-canonical/data/ast.yaml`,
  `crates/handl-kit-data-canonical/data/eval.yaml` (if a new node kind is
  added).

### 0.2 — Recursor `if` has 5 children (eval expects 3)  (Bug #2)

- **Where:** `crates/handl-runtime/src/desugar_pass.rs:980-983`
  (`build_recursor_dispatch`).
- **Bug:** The else-if chain is emitted as a flat 5-child `if`:
  `if{children: [cond, then, else_cond, else_then, else_else]}`. The `if`
  eval scheme expects exactly 3 children `[cond, then, else]`. At eval time
  this either errors or silently ignores the extra children.
- **Fix:** Nest the else-if. The else slot must itself be an `if` node:
  `if{[first_tag_eq, base, if{[second_tag_eq, step_expr, null_lit]}]}`.
  Refactor `build_recursor_dispatch` to fold constructors right-to-left
  into nested `if` nodes (this also generalizes to N constructors — see
  Phase 2.1).
- **Test:** Unit test asserting the generated `if` tree is binary-nested
  (every `if` node has exactly 3 children). Eval test: `Nat.rec(0, step)(Zero)`
  returns `0`; `Nat.rec(0, step)(Succ(Zero))` returns `step(Zero, 0)`.
- **Files:** `crates/handl-runtime/src/desugar_pass.rs`.

### 0.3 — Recursor can't recurse (`assign` is non-recursive)  (Bug #3)

- **Where:** `crates/handl-runtime/src/desugar_pass.rs:832-834`.
- **Bug:** `Nat.rec = (base, step) -> ...` is emitted as an `assign` whose
  target is `var("Nat.rec")`. `assign` creates a non-recursive binding, so
  `Nat.rec` is **not in scope inside its own body**. The recursive call
  `Nat.rec(base, step)(n.pred)` would hit an unbound-variable error at
  eval time. The generated recursor is dead code for any non-zero input.
- **Fix:** Use a recursive binding form. Options:
  1. If a `rec` node kind exists (ADR-0082 §3 mentions "like `rec` nodes"),
     emit `rec{[target, value]}` instead of `assign{[target, value]}` for
     the recursor binding only. Add `rec` to `ast.yaml`/`eval.yaml` if
     missing.
  2. If `block` supports mutual recursion among its bindings, emit the
     recursor as a `block`-local recursive binding. Check whether the
     `block` eval scheme already supports this; if not, that's a separate
     eval change.
- **Test:** Eval test that actually exercises recursion:
  `Nat.rec(0, (pred, rec_pred) -> Succ(rec_pred))(Succ(Succ(Zero)))`
  returns `Succ(Succ(Zero))`. This is the minimal test that proves the
  recursor can recurse.
- **Files:** `crates/handl-runtime/src/desugar_pass.rs`,
  `crates/handl-kit-data-canonical/data/ast.yaml`,
  `crates/handl-kit-data-canonical/data/eval.yaml` (if `rec` kind added).

### 0.4 — `data` desugar error silently swallowed  (Bug #13)

- **Where:** `crates/handl-runtime/src/desugar_pass.rs:765-773`.
- **Bug:** On strict-positivity violation, `desugar_data` emits a `lit`
  node with payload `"__data_error__: <reason>"` and returns `Some(...)`.
  Nothing downstream checks for the `__data_error__` prefix — the bad
  `data` decl silently becomes a string literal, and the error reason is
  lost. The user sees nothing.
- **Fix:** The desugar pass is a pure `Ast → Ast` transform and can't
  surface compile errors directly (per the comment at line 767-770). So:
  1. Define a real error-node kind (e.g. `desugar-error`) in `ast.yaml`
     whose payload carries `{kind, message, span?}`.
  2. Have the typechecker/eval recognize `desugar-error` nodes and emit a
     proper `LangError` / diagnostic with the reason.
  3. Replace the `lit` + `__data_error__` hack with a `desugar-error` node.
- **Test:** Desugar `data Bad = Cons(Bad -> Int)` and assert the output
  contains a `desugar-error` node with the positivity message. Eval test
  that evaluating a `desugar-error` node produces a `LangError` whose
  message contains the reason.
- **Files:** `crates/handl-runtime/src/desugar_pass.rs`,
  `crates/handl-kit-data-canonical/data/ast.yaml`,
  `crates/plugin-engine-eval*/src/` (recognize the error node),
  `crates/plugin-engine-typecheck*/src/` (recognize for diag).

---

## Phase 1 — Make `data` reachable from surface syntax

The desugar fires on `"data"` kind nodes, but there is no `ast.data`
shape, no `grammar.data` rule, and no eval scheme in the canonical YAML.
Only hand-built AST nodes reach the desugar. The feature is unreachable
from handl source. This must land before any of the Phase 0 fixes can be
tested end-to-end from source.

### 1.1 — Add `data` grammar/AST/eval entries  (Gap #4)

- **Where:** `crates/handl-kit-data-canonical/data/ast.yaml`,
  `grammar.yaml`, `eval.yaml` (all currently have no `data` entries).
- **Task:**
  1. `ast.yaml`: declare `AstShapeDef` for `data` with named child roles
     per ADR-0081 — `name` (the type name), `constructors` (the constructor
     list). Decide whether constructors are a single list child or
     repeated children; match whatever the parser produces.
  2. `grammar.yaml`: add a `data` parse rule. Surface form (per ADR-0082
     §1): `data Name = Cons1 | Cons2(args) | ...`. This is a top-level
     declaration; integrate with the existing declaration parser.
  3. `eval.yaml`: `data` has no direct eval semantics (it desugars away),
     but if the desugar pass is *runtime* (runs in `Runtime::parse`), the
     eval engine may still see `data` nodes if desugar is skipped. Decide:
     - If desugar is a compile-time pass (before eval), no eval scheme
       needed — but document this.
     - If desugar runs at runtime, add a no-op or error eval scheme so an
       undessugared `data` node doesn't crash.
  4. Verify the parser round-trips `data Nat = Zero | Succ(Nat)` into the
     AST shape `desugar_data` expects (`parse_data_payload` at line 762
     reads the payload — make sure the parser produces that shape).
- **Test:** Parse `data Nat = Zero | Succ(Nat)` from source, run the
  desugar pass, and eval `Succ(Zero)` → `{"tag": "Succ", "pred": {"tag": "Zero"}}`.
  This is the first end-to-end `data` test from surface syntax.
- **Files:** `crates/handl-kit-data-canonical/data/{ast,grammar,eval}.yaml`,
  `crates/plugin-engine-parse*/src/` (if the grammar rule needs parser
  support), `crates/handl-runtime/src/desugar_pass.rs` (verify
  `parse_data_payload` matches the parsed shape).

### 1.2 — Resolve `data` desugar-before-typecheck coupling  (Gap #14)

- **Where:** `crates/handl-runtime/src/desugar_pass.rs` runs inside
  `Runtime::parse` (per the module doc, "Runs between parse and eval").
- **Issue:** The `data` desugar generates predicate/constructor/recursor
  definitions that the typechecker needs to see (e.g. to recognize `Nat`
  as a predicate). If desugar runs at runtime (after typecheck), the
  typechecker sees raw `data` nodes and can't resolve `Nat`. If desugar
  runs before typecheck, the typechecker depends on a runtime pass — a
  layering coupling.
- **Task:** Decide and document where the desugar pass runs in the
  pipeline:
  - **Option A (preferred):** Make desugar a compile-time pass invoked by
    the typechecker host before typechecking. The typechecker never sees
    `data` nodes. Document this in ADR-0082.
  - **Option B:** Keep desugar at runtime, but have the typechecker
    invoke the desugar on type expressions that reference `data`-defined
    types. More coupling, less clean.
  - Either way, add a test that typechecks `x : Nat = Succ(Zero)` from
    source (proving the typechecker sees the desugared predicate).
- **Files:** `crates/plugin-engine-typecheck*/src/`,
  `crates/handl-runtime/src/`, ADR-0082 (document the decision).

---

## Phase 2 — Recursor correctness & generality

Depends on Phase 0 (the recursor must actually run before generalizing
it). These close the "initial implementation" limitations noted in the
code comments.

### 2.1 — Recursor handles N constructors  (Gap #6)

- **Where:** `crates/handl-runtime/src/desugar_pass.rs:955-982`
  (`build_recursor_dispatch` hardcodes first + second constructor).
- **Bug:** A 3-constructor type like `data RGB = Red | Green | Blue` gets
  a recursor that only dispatches on Red/Green — Blue is unreachable. The
  comment at line 826-829 says "initial implementation" but it's a silent
  correctness gap.
- **Fix:** Generalize `build_recursor_dispatch` to fold over *all*
  constructors, building a right-nested `if` tree (this overlaps with
  Phase 0.2 — do them together). Each non-base constructor branch calls
  its step function with its recursive args and the recursive
  `Type.rec(...)` application on each recursive arg. Constructors with
  multiple recursive args need multiple recursive calls (one per
  recursive arg).
- **Test:** `data RGB = Red | Green | Blue`; recursor
  `RGB.rec("r", "g", "b")` returns `"r"`/`"g"`/`"b"` for the three
  constructors. `data Tree = Leaf | Node(Tree, Tree)` with a recursor
  that sums both subtrees — exercises multi-arg recursion.
- **Files:** `crates/handl-runtime/src/desugar_pass.rs`.

### 2.2 — Strict positivity check walks type-expression AST  (Gap #5)

- **Where:** `crates/handl-runtime/src/desugar_pass.rs:726-747`
  (`check_strict_positivity`).
- **Bug:** The check splits the arg *type string* on `"->"` and compares
  the domain substring to the type name. It doesn't handle:
  - Nested function types: `(Bad -> Int) -> Int` (the domain is
    `Bad -> Int`, not `Bad`, so the split-and-compare misses it).
  - Parameterized types: `List(List(Nat))` (no `->` at all, but still
    needs checking if `List` is the type being defined).
  - Any actual type-expression AST — it string-matches.
- **Fix:** Parse the arg type into a type-expression AST (or have the
  parser produce one in the first place — see Phase 1.1). Walk the tree:
  - A function type `A -> B`: `A` is in *negative* position; recurse into
    `A` flipping polarity, recurse into `B` keeping polarity.
  - A type application `F(args)`: recurse into each arg in positive
    position (or negative if the polarity is flipped).
  - The type being defined (`Name`) in a negative position → violation.
- **Test:** `data Bad = Cons(Bad -> Int)` → rejected.
  `data Bad2 = Cons((Bad2 -> Int) -> Int)` → rejected (nested).
  `data Ok = Cons(Int -> Ok)` → accepted (Ok only in positive position).
- **Files:** `crates/handl-runtime/src/desugar_pass.rs`. May need a
  type-expression parser/AST if one doesn't exist (check
  `plugin-engine-parse*` and `kit-type`).

---

## Phase 3 — B2 symbolic normalizer gaps

ADR-0068 step B2 (symbolic normalization for predicates with free
variables) has several coverage gaps. These are independent of the
`data` track but block real-world predicate typechecking.

### 3.1 — B2 handles named/parametric predicates  (Gap #7)

- **Where:**
  `crates/plugin-engine-typecheck-nbe/src/predicate_eval.rs:372-377`.
- **Bug:** `check_predicate_symbolic` rejects `kind != "function"` with
  `Blocked("expected function node, got 'call'")`. But when the type
  expression is `DivisibleBy(3)` (a `call` that produces a closure), the
  node kind is `call`, not `function`. B2 only works for inline
  `(n) -> ...` literals.
- **Fix:** Before symbolic checking, *evaluate* the type expression via
  the eval engine to get the closure value (B1 already does this at
  lib.rs:1646-1648). The closure carries a `body` index into the AST it
  was created from. Use that body (and the closure's param binding) as
  the predicate body for B2, instead of requiring a raw `function` node.
  This unifies the B1 and B2 entry points: both start from a closure
  value, B1 applies it, B2 reads its body and normalizes symbolically.
- **Test:** `DivisibleBy(3)` defined as `(k) -> (n) -> n % k == 0`;
  typecheck `x : DivisibleBy(3) = 6` → Proven (B2 normalizes
  `6 % 3 == 0` → `0 == 0` → `true`). With a free var:
  `x : DivisibleBy(3) = y` → Blocked (B2 can't prove `y % 3 == 0`), not
  an error.
- **Files:** `crates/plugin-engine-typecheck-nbe/src/predicate_eval.rs`,
  `crates/plugin-engine-typecheck-nbe/src/lib.rs` (the fallback wiring).

### 3.2 — B2 handles `params`/`body` payload shape  (Bug #8)

- **Where:**
  `crates/plugin-engine-typecheck-nbe/src/predicate_eval.rs:378-388`.
- **Bug:** `check_predicate_symbolic` reads children from
  `payload.get("children")` only. But the `closure` native op and
  `synth_function` both handle a second shape: `payload.get("params")` +
  `payload.get("body")`. If the parser produces that shape, B2 returns
  `Blocked("predicate function has < 2 children")` — silently falling
  back when it shouldn't.
- **Fix:** Mirror the shape-extraction logic from `synth_function` /
  `closure` eval. Read `params` + `body` if present, else fall back to
  `children[0]` + `children[1]`. Better: extract a helper
  `function_param_body(ast, node) -> Option<(param_idx, body_idx)>` and
  use it in both B2 and the eval closure op (DRY + ensures they agree).
- **Test:** Construct a `function` node with the `params`/`body` payload
  shape and run `check_predicate_symbolic` — must not return the
  `< 2 children` Blocked.
- **Files:** `crates/plugin-engine-typecheck-nbe/src/predicate_eval.rs`.

### 3.3 — B2 supports `neq`/`lt`/`gt`/`lte`/`gte`/`if`/`call`  (Gap #15)

- **Where:**
  `crates/plugin-engine-typecheck-nbe/src/predicate_eval.rs:236-284`
  (`ast_to_sym`).
- **Bug:** `ast_to_sym` handles only `lit`, `var`, `binary.add`,
  `binary.sub`, `binary.mul`, `binary.mod`, `binary.eq`, `binary.and`,
  `binary.or`. Missing: `binary.neq`, `binary.lt`, `binary.gt`,
  `binary.lte`, `binary.gte`, `if`, `call`. Any predicate using these
  hits the catch-all `_ => SymExpr::stuck(...)` and B2 falls back to B1.
- **Fix:** Add `SymExpr` variants for the missing comparisons (`Neq`,
  `Lt`, `Gt`, `Lte`, `Gte`) and `If` (conditional) and `Call`
  (application). Add `normalize_step` rules:
  - Comparisons on concrete ints → `Bool`.
  - `If(Bool(c), t, e)` → `c ? t : e`.
  - `Call` is harder — only normalize if the callee is a known closure
    and the arg is concrete (beta-reduce). Otherwise `Stuck`.
- **Test:** Per-kindle tests: `(n) -> n != 0` with `n = 0` → Proven;
  `(n) -> n < 10` with `n = 5` → Proven; `(n) -> if n == 0 then true
  else false` with `n = 0` → Proven.
- **Files:** `crates/plugin-engine-typecheck-nbe/src/predicate_eval.rs`.

### 3.4 — `Stuck` propagates through operations  (Minor #11)

- **Where:**
  `crates/plugin-engine-typecheck-nbe/src/predicate_eval.rs:98-106`
  (and every other binary op's fallback arm).
- **Bug:** `Add(Stuck("..."), Int(1))` — the normalizer tries
  `normalize_step` on each child (`Stuck → None`), then on `Add` itself
  (neither side is `Int` → `None`). Result: `Add(Stuck, Int(1))` stays
  unreduced. It maps to `Blocked` via the catch-all at line 415-418, so
  it's not a *correctness* bug, but the `Stuck` reason message is lost —
  the user sees `"predicate did not reduce to bool, got: Add(Stuck(...),
  Int(1))"` instead of the actual stuck reason.
- **Fix:** In each binary op's fallback arm, if either child is `Stuck`,
  return `Some(Stuck(<propagated reason>))` instead of trying to
  re-wrap. Add a helper `propagate_stuck(a, b) -> Option<SymExpr>`.
- **Test:** `(n) -> n + 1 == 1` with `n = <unbound>` → Blocked with a
  message containing the stuck reason from the `n` lookup, not the
  `Add(...)` wrapper description.
- **Files:** `crates/plugin-engine-typecheck-nbe/src/predicate_eval.rs`.

### 3.5 — Remove unused `ast` param in `binary_children`  (Trivial #12)

- **Where:**
  `crates/plugin-engine-typecheck-nbe/src/predicate_eval.rs:288-298`.
- **Bug:** `fn binary_children(ast: &Ast, node: &AstNode)` — `ast` is
  never used; only `node.payload` is read. Dead parameter.
- **Fix:** Drop the `ast` param; update all 7 call sites (lines 256, 260,
  264, 268, 272, 276, 280). Trivial cleanup.
- **Files:** `crates/plugin-engine-typecheck-nbe/src/predicate_eval.rs`.

### 3.6 — B1→B2 fallback end-to-end test  (Gap #9)

- **Where:** The fallback wiring lives at
  `crates/plugin-engine-typecheck-nbe/src/lib.rs:1600-1620` and
  `1661-1668`. Tests at `tests.rs:9450+` call `check_predicate_symbolic`
  *directly*, bypassing `synth_type_annot`. The actual fallback (try B1,
  if Blocked try B2, use B2 only if better) has **zero** e2e coverage.
- **Task:** Add tests that drive `synth_type_annot` (the real entry
  point) end-to-end:
  1. B1 Proven → no B2 call (assert B2 not invoked via a counter or mock).
  2. B1 Blocked, B2 Proven → result is Proven (B2 rescued).
  3. B1 Blocked, B2 Blocked → result is Blocked (B2 didn't help).
  4. B1 Disproved → no B2 call (B1 already decided).
  - These need to go through the full `synth_type_annot` scheme
    dispatch, not call the helpers directly.
- **Files:** `crates/plugin-engine-typecheck-nbe/src/tests.rs`.

### 3.7 — B1/B2 agreement test on concrete cases  (Gap #16)

- **Where:** No existing test.
- **Task:** For predicates where both B1 and B2 can run (concrete
  expressions, no free vars), assert they agree:
  - Both Proven, or both Disproved, or both Blocked — never one Proven
    and the other Disproved.
  - Property test: generate random concrete int expressions, run both
    B1 and B2 on `(n) -> n % k == 0` style predicates, assert agreement.
  - This is a regression guard: if B1 and B2 ever disagree on a concrete
    case, one of them is wrong.
- **Files:** `crates/plugin-engine-typecheck-nbe/src/tests.rs`.

---

## Phase 4 — Engine-genericity violations (tech debt)

Per the project rule (`.devin/rules/engine-genericity.md`), engines must
not hardcode `kind ==` checks. Two violations were shipped with the
ADR-0068/0082 work. These are tech debt to *close*, not patterns to
extend.

### 4.1 — `desugar_data` hardcoded kind dispatch  (Tech debt #10a)

- **Where:** `crates/handl-runtime/src/desugar_pass.rs:1014`
  (`"data" => desugar_data(original, out)` in `try_rust_desugar`).
- **Bug:** Hardcoded `match` arm on `"data"`. Per the rule, this should
  be data-driven: extend `AstDesugarRule` to support one-to-many
  expansion (a `data` node expands to a `block` of multiple definitions).
- **Fix:** Extend `AstDesugarRule` (in `handl-language`) with a
  one-to-many expansion variant — e.g. `DesugarRule::ExpandMany { target:
  "block", children: <recipe> }` where the recipe references the original
  node's named roles (`name`, `constructors`) and declares how to build
  each output statement. Move the `desugar_data` logic into a
  data-driven rule + a small Rust expansion primitive that the rule
  invokes. The `"data"` arm in `try_rust_desugar` should disappear.
- **Note:** This is the hardest item here — it requires extending the
  desugar rule schema. Defer until Phase 0–3 are stable; the hardcoded
  arm works, it's just a rule violation. Flag it in any PR that touches
  `desugar_pass.rs`.
- **Files:** `crates/handl-language/src/language.rs` (AstDesugarRule),
  `crates/handl-runtime/src/desugar_pass.rs`,
  `crates/handl-kit-data-canonical/data/ast.yaml` (the rule data).

### 4.2 — `ast_to_sym` / `check_predicate_symbolic` hardcoded kind checks  (Tech debt #10b)

- **Where:**
  `crates/plugin-engine-typecheck-nbe/src/predicate_eval.rs:236-284`
  (`ast_to_sym` match on `lit`/`var`/`binary.add`/...).
- **Bug:** Same rule violation as 4.1 — hardcoded `match` on node kinds
  inside the typechecker. The symbolic translator should be data-driven:
  each node kind's `TypeRuleDef` (or a new `SymRuleDef`) declares how it
  maps to a `SymExpr`.
- **Fix:** Add a `SymRuleDef` (or extend `TypeRuleDef`) in `handl-language`
  that declares, per node kind, the `SymExpr` constructor + which child
  roles to read. `ast_to_sym` becomes a data-driven dispatch over
  `lang.types[node.kind].sym_rule` (or similar). The hardcoded match
  disappears.
- **Note:** Like 4.1, this is a schema extension. Defer until Phase 3
  (B2 feature gaps) are closed — the kind set is still changing, so
  data-driving it now would churn. But flag it in every PR that adds a
  new arm to `ast_to_sym`.
- **Files:** `crates/handl-language/src/language.rs`,
  `crates/plugin-engine-typecheck-nbe/src/predicate_eval.rs`,
  `crates/handl-kit-data-canonical/data/types.yaml` (the rule data).

---

## Phase 5 — Runtime-assert IR emission  (Track item 1)

ADR-0068 §1 "graduated guarantees": when the typechecker can't normalize a
predicate annotation (Verdict::Blocked), it accepts the annotation
*statically* but records a `runtime_assert` diagnostic so the runtime
emits an actual `assert T(e)` at that site. The diagnostic is emitted
(`predicate_eval.rs:457-459`) but **nothing consumes it** — no
`assert T(e)` node is ever inserted into the IR/AST.

### 5.1 — Wire `runtime_assert` diagnostics into the eval/runtime pipeline

- **Where:** Diagnostic emitted at
  `crates/plugin-engine-typecheck-nbe/src/predicate_eval.rs:457-459`.
  No consumer (grep of `handl-runtime/` + `plugin-engine-eval*/` for
  `runtime_assert` returns nothing).
- **Task:**
  1. **Define the runtime-assert node.** Add an `assert` (or
    `runtime-assert`) node kind to `ast.yaml` with roles `predicate` and
    `expr`. Its eval scheme: evaluate `predicate(expr)`; if the result is
    not `true`, raise a `LangError` (runtime assertion failure). If
    `true`, evaluate to `expr`'s value (pass-through).
  2. **Collect `runtime_assert` diagnostics after typecheck.** The
    typecheck host (or a post-typecheck pass) reads the diagnostics
    emitted during `synth_type_annot` and, for each, inserts an `assert`
    node wrapping the annotated expression. The insertion point is the
    `type-annot` node's expression child.
  3. **Decide where insertion happens.** Options:
     - In the typechecker host, before handing the AST to eval (the
       typechecker rewrites the AST).
     - In a separate "lowering" pass between typecheck and eval.
     - Document the choice; prefer the one that keeps the typechecker
       side-effect-free on the AST (a separate pass is cleaner).
  4. **Eval the assert node.** Add the eval scheme so the runtime
    actually checks the predicate at runtime and fails loudly on
    violation.
- **Test:**
  - Typecheck `x : (n) -> n > 0 = y` (free var `y` → Blocked →
    runtime_assert emitted). Eval `x` with `y = 5` → `5` (assert passes).
    Eval with `y = -1` → `LangError` (assertion failure, message names
    the predicate).
  - Typecheck `x : (n) -> n > 0 = 5` (concrete → Proven → no
    runtime_assert). Assert no `assert` node in the lowered AST.
- **Files:** `crates/handl-kit-data-canonical/data/{ast,eval}.yaml`,
  `crates/plugin-engine-eval*/src/` (assert eval scheme),
  `crates/plugin-engine-typecheck*/src/` or a new lowering pass,
  `crates/handl-runtime/src/` (pipeline wiring).
- **Closes:** ADR-0068 Proof/axiom row.

---

## Phase 6 — ADR-0082 steps 3+  (Track item 2)

ADR-0082 build order (lines 200-212):
1. `data` desugaring — **done** (Phase 0–2 above fix its bugs).
2. Strict positivity check — **done** (Phase 2.2 makes it real).
3. Checker recognition of generated recursors for termination.
4. `match` as sugar over the recursor.
5. Inductive families (value-indexed predicates: `Vec`, `Fin`).
6. Dependent pattern matching with motive argument.

Steps 3+ are unblocked once Phase 0–2 land (the recursor actually works).

### 6.1 — Checker recognizes generated recursors for termination  (Step 3)

- **Task:** The typechecker must recognize the generated `Type.rec`
  recursors as *structurally recursive* so they pass termination
  checking (when a `Total` predicate lands — see Phase 7.1). The
  recursor decreases on the inductive argument (`n.pred`); the checker
  must see this without a hardcoded `Type.rec` name match.
- **Approach (data-driven):** When `data` desugars, emit a metadata
  entry (in the desugar output or a side table) declaring "this function
  is structurally recursive on argument N, decreasing via field M". The
  termination checker reads this metadata. No `kind == "Type.rec"` check.
- **Files:** `crates/handl-runtime/src/desugar_pass.rs` (emit metadata),
  `crates/plugin-engine-typecheck*/src/` (consume it),
  `crates/handl-language/src/language.rs` (metadata schema).

### 6.2 — `match` as sugar over the recursor  (Step 4)

- **Task:** Surface `match` syntax desugars to recursor application. E.g.
  `match n { Zero => 0, Succ(pred) => 1 + f(pred) }` desugars to
  `Nat.rec(0, (pred, rec) => 1 + f(pred))(n)`.
- **Depends on:** Phase 0.3 (recursor can recurse), Phase 2.1 (N
  constructors).
- **Files:** `crates/handl-runtime/src/desugar_pass.rs`,
  `crates/handl-kit-data-canonical/data/{ast,grammar}.yaml` (`match`
  shape + parse rule).

### 6.3 — Inductive families  (Step 5)

- **Task:** Value-indexed predicates (`Vec n a`, `Fin n`). These are
  predicates that take the index as an argument and enforce the
  length-indexed invariant. The `data` desugar must generate
  parameterized predicates, not just tag-checks.
- **Depends on:** Phase 2.1, 2.2. This is a significant extension of
  `desugar_data`; likely needs its own ADR amendment.

### 6.4 — Dependent pattern matching with motive  (Step 6)

- **Task:** Full dependent eliminator — `match` with a motive argument
  that refines the return type per branch. This is the full
  dependently-typed layer.
- **Depends on:** 6.1–6.3. Largest item in this track; likely needs a
  dedicated workstream.

---

## Phase 7 — `Total` & `Pure` predicates  (Track items 3, 4)

### 7.1 — `Total` predicate — structural recursion checking  (Track 3)

- **What:** An on-demand predicate that asserts a function is total
  (terminates on all inputs) by checking it's structurally recursive.
- **Approach:** When a function is annotated `Total` (or
  `f : Total(Int -> Int)`), the typechecker analyzes its body for
  structural recursion: every recursive call must be on a strict
  sub-structure of an argument (e.g. `n.pred` is a sub-structure of
  `n`). Reuse the metadata from Phase 6.1 for generated recursors; for
  user-written recursive functions, do a syntactic check (call graph +
  decreasing argument).
- **Depends on:** Phase 6.1 (recursor recognition) for the generated
  case; the user-written case is independent.
- **Files:** `crates/plugin-engine-typecheck*/src/` (new
  `totality_check` module), `crates/handl-kit-data-canonical/data/`
  (`Total` type declaration).

### 7.2 — `Pure` predicate — effect tracking for purity  (Track 4)

- **What:** An on-demand predicate that asserts a function is pure
  (effect-free). The typechecker already tracks effect rows
  (`plugin-engine-typecheck-nbe/src/lib.rs:325`); `Pure` exposes this as
  a predicate the user can assert.
- **Approach:** `f : Pure(Int -> Int)` checks that `f`'s effect row is
  empty. This is mostly wiring: the effect row is already computed; the
  `Pure` predicate reads it and returns Proven/Disproved.
- **Files:** `crates/plugin-engine-typecheck*/src/` (Pure predicate
  rule), `crates/handl-kit-data-canonical/data/` (`Pure` type
  declaration).

---

## Phase 8 — FFI track  (Track items 5, 6)

### 8.1 — Cranelift Foreign dispatch  (Track 5)

- **Current state:** `plugin-ffi` uses `libloading` (`dlopen`/`dlsym`)
  with a hand-rolled call-dispatch macro for arities 0–6. It plugs into
  `OpRegistry` as a `Function`. `ModuleLookup` trait objects still live
  in `plugin-module` — **not replaced**.
- **Task:** Replace the `dlopen`/`dlsym` + macro-dispatch with a
  Cranelift-based foreign-call shim: Cranelift generates a native call
  stub per FFI op signature, avoiding the arity-macro ceiling (currently
  6) and enabling JIT'd trampolines. Then replace `ModuleLookup` trait
  objects in `plugin-module` with the Cranelift dispatcher (foreign calls
  become data, not trait dispatch).
- **Depends on:** ADR-0083 Phase 1+ (the ADR mentions Cranelift as the
  long-term dispatch path). Check whether `plugin-engine-eval-cranelift`
  already has reusable JIT infrastructure.
- **Files:** `crates/plugin-ffi/src/` (rewrite dispatcher),
  `crates/plugin-module/src/` (remove `ModuleLookup` trait objects),
  `crates/plugin-engine-eval-cranelift/src/` (reuse JIT).
- **Note:** This is a large refactor; the current `dlopen` path works
  for Phase 0. Defer until ADR-0083 Phase 1 (POSIX layer) needs >6 args
  or perf becomes a blocker.

### 8.2 — Typechecker marshalability  (Track 6)

- **What:** The typechecker must know which handl types are marshalable
  across the FFI boundary (`cstr`, `i32`, `i64`, `f64`, `ptr`, `Bytes`).
  Currently `marshal.rs` has the descriptors but the typechecker has no
  `ffi`/`marshal`/`c_type` integration (grep returns nothing).
- **Task:**
  1. Declare marshalable types in canonical type data (`cstr`, `i32`,
     `i64`, `f64`, `ptr`, `Bytes`) with their C-ABI mapping.
  2. Add a typechecker rule for `ffi_bind` op declarations that checks
     each argument type is marshalable and the return type is
     marshalable (or `void`). Reject non-marshalable types (e.g.
     closures, records) at the FFI boundary with a clear error.
  3. Add a `Marshalable` predicate so users can assert a type is
     FFI-safe: `T : Marshalable`.
- **Files:** `crates/handl-kit-data-canonical/data/types.yaml`,
  `crates/plugin-engine-typecheck*/src/` (ffi_bind type rule),
  `crates/plugin-typecheck*/src/` (Marshalable predicate).

---

## Phase 9 — Tooling scaffolds  (Track items 7, 8)  [x — superseded by ADRs 0085-0091, implemented 2026-09-04]

### 9.1 — Formatter — handl rules for `BasicFormatter`  (Track 7)  [x — superseded by ADR-0086; `plugin-ext-fmt` implements `format_source`/`check_source` with canonical style, golden tests, idempotence]

- **Where:** `crates/platform/src/format.rs` — `BasicFormatter::new()`
  starts with empty `rules`. No fmt YAML in
  `crates/handl-kit-data-canonical/data/`.
- **Task:** Add `FormatBehavior` rules for every handl construct (per
  ADR-0074 surface syntax): `let`, `if`, `assign`, `block`, `function`,
  `call`, `binary.*`, `data`, `match`, etc. Each rule declares the
  layout (indentation, line breaks, spacing). Drive from canonical data
  (`data/fmt.yaml`) so adding a language construct adds its format rule
  — no Rust edits.
- **Note:** ADR-0086 (bootstrap formatter) may supersede this — check
  whether the bootstrap `tooling-fmt` crate is the intended path
  instead. If so, close this as "superseded by ADR-0086" and track there.
- **Files:** `crates/platform/src/format.rs`,
  `crates/handl-kit-data-canonical/data/fmt.yaml` (new).

### 9.2 — Linter — `plugin-diag-expression` is a no-op  (Track 8)  [x — superseded by ADR-0087; `plugin-ext-lint` implements 7 canonical rules with shared diagnostics. `plugin-diag-expression` kept for backward compat but option 2 (supersede) is the decision]

- **Where:** `crates/plugin-diag-expression/src/lib.rs` — `apply` is
  `Ok(())` with a TODO: "wire expression-specific diagnostics through
  the generic `LintRequest` once an expression payload is available."
- **Task:** Either:
  1. Wire expression data through `LintRequest` (requires extending the
     `LintRequest` payload to carry an AST/expression — a platform
     change), then implement expression diagnostics (unused vars,
     unreachable branches, etc.). Or
  2. Mark this crate as superseded by ADR-0087 (bootstrap linter
     `tooling-linter`) and delete it in Phase 3 of TODO.md's legacy
     deletion. Prefer this if the bootstrap path is the plan.
- **Note:** Check ADR-0087 status before investing in (1).
- **Files:** `crates/plugin-diag-expression/src/lib.rs`,
  `crates/platform/src/` (LintRequest payload, if option 1).

---

## Phase 9.5 — `import` statement has no engine handler  (Feature gap)

### 9.5.1 — `import` (and `export`) Foreign ops are unimplemented

- **Where:**
  - Grammar: `crates/handl-kit-data-canonical/data/grammar.yaml:227-234`
    — `grammar.stmt-import.prefix` parses `import "path"` into a
    `stmt.import` node (arity 1).
  - AST: `crates/handl-kit-data-canonical/data/ast.yaml:385-397` —
    `ast.import` declares the node shape with `path` (string, required)
    and `items` (array of strings, optional — the imported names).
  - Eval: `crates/handl-kit-data-canonical/data/eval.yaml:492-502` —
    `eval.stmt-import` is `!Arr { op: import, args: [proj_child(0)],
    source: !Foreign import }`. The `export` op at line 481-491 is
    identical: `source: !Foreign export`.
  - Engine dispatch: `crates/plugin-engine-eval-tree-walk/src/lib.rs:750-766`
    — `ArrSource::Foreign(name)` dispatches via
    `modules.lookup(name)`. For `"import"` and `"export"`, no module
    registers these names, so `lookup` returns `None` →
    `LangError::engine_node("unregistered foreign: import", node)`.
  - Grep confirms: zero hits for `"import"` or `"export"` as registered
    module functions across `plugin-lib-*/src/`, `plugin-module/src/`,
    `kit-stdlib/src/`, `kit-canonical/src/`.
- **Bug:** The grammar parses `import "path"`, the eval YAML says "call
  the `import` Foreign op", but **no engine has a handler for the
  `import` (or `export`) op**. Both fall through to "unregistered
  foreign" and error. You literally cannot write `import` in a program
  today — it parses but can't evaluate.
- **Root cause:** The `import`/`export` ops were declared in the
  canonical data (grammar + AST + eval scheme) but the implementation —
  a module that resolves the path, loads the file, parses/evaluates it,
  and binds the imported names into the current scope — was never built.
  This is a data-declared feature with no Rust backing.
- **Fix:** Implement the `import` Foreign op as a registered module
  function (or a dedicated handler). The work breaks down:
  1. **Path resolution.** The `import` node's child (or `path` payload
     field) is a string path. Resolve it relative to the importing
     file's directory (or a configured module path). The `ast.import`
     shape has `path` (required) and `items` (optional list of names).
  2. **File loading.** Read the resolved file from disk. This requires
     filesystem access — the eval engine currently has none. Options:
     - Register an `import` module function in `plugin-module` or a new
       `plugin-lib-import` crate that has fs access. The function
       receives the path string, reads the file, lexes/parses/evaluates
       it in a sub-runtime, and returns the module's exported bindings
       as a `Value` (object/record).
     - Or add `import` as a `Native` op (not `Foreign`) in the tree-walk
       engine directly, giving it access to the `Runtime` for
       recursive evaluation. This is less data-driven but simpler.
     - Prefer the module approach to keep engines generic.
  3. **Scope binding.** The imported names (`items` in the AST payload)
     must be bound into the current scope. If `items` is empty or
     absent, import all exports. The `export` op must collect bindings
     from the current scope and tag them as exported.
  4. **`export` op.** Symmetric: collect the current module's exported
     bindings so `import` can retrieve them. The `export` node's child
     is the value to export; the op registers it under the module's
     export table.
  5. **Module caching.** Avoid re-evaluating the same import twice.
     Cache by resolved path. This may need a module cache in
     `ModuleRegistry` or the `Runtime`.
  6. **Cycle detection.** Detect circular imports and error clearly
     (don't infinite-loop).
- **Design question:** Does `import` belong in the eval engine (runtime)
  or in a compile-time pass (like `data` desugar)? ADR-0077 (stdlib
  migration) treats imports as a compile-time module-resolution step.
  If imports are resolved at compile time, the eval engine should never
  see `stmt.import` nodes — they'd be desugared to the imported
  bindings. Check ADR-0077 and ADR-0074 for the intended semantics
  before implementing. If compile-time, this becomes a desugar pass
  (like Phase 1.1 for `data`); if runtime, it's a Foreign op.
- **Test:**
  - `import "math.hnl"` in a file that uses `math.add` → the import
    resolves, `math.add` is in scope, the program evaluates.
  - `import { add } from "math.hnl"` (named import) → only `add` is
    bound.
  - Circular import (`a` imports `b`, `b` imports `a`) → clear error,
    not a hang.
  - Missing file → clear error with the path.
  - `export` in a module → the exported name is visible to importers;
    non-exported names are not.
- **Files:**
  - `crates/plugin-module/src/lib.rs` or new `crates/plugin-lib-import/`
    (the `import`/`export` module function).
  - `crates/handl-runtime/src/lib.rs` (wire the module into the
    registry; possibly recursive eval for the imported file).
  - `crates/handl-kit-data-canonical/data/eval.yaml` (may need to change
    `Foreign` → `Native` if the approach changes).
  - `crates/plugin-engine-eval-tree-walk/src/lib.rs` (if Native op
    approach).
  - Check `project/adrs/0077-stdlib-migration-to-handl.md` and
    `project/adrs/0074-*` for the intended import semantics.

### 9.5.2 — No handl-source module loader for the eval runtime

- **Where:** `crates/handl-runtime/src/lib.rs` (the eval runtime) has
  `ModuleRegistry` (`crates/plugin-module/src/lib.rs:69`) for
  Rust-registered host functions only. No `load_module(path)`, no
  `resolve_module(name)`, no file-path resolution, no caching of parsed
  handl-source modules. Programs are single-file only.
- **What exists (but in the wrong place):**
  `crates/plugin-ext-refinement/src/arrow_loader.rs` has a complete
  module-loading infrastructure:
  - `ModuleResolver` trait (line 304) — `resolve(&self, path) ->
    Result<ResolvedModule, ResolveError>`.
  - `FileResolver` (line 327) — resolves `<root>/<path>.hnl` from disk.
  - `MemoryResolver` (line 386) — in-memory module map (for tests).
  - `ChainResolver` (line 410) — tries resolvers in order.
  - `CachingResolver` — wraps another resolver with a cache.
  - `ModuleLoader` (line 523) — `load(path) -> Result<LoadedModule,
    LoadError>`, with two caches (source + parsed arrows), cycle
    detection via `load_transitive` (line 1163).
  - `flatten_program` (line 1016) — eliminates module refs, producing a
    self-contained `Scheme`.
  - **But:** this all works on arrow-graph `Scheme`s (the compiler IR),
    not on handl-source ASTs. It's used only by `hnlc` (the compiler
    CLI, `plugin-ext-refinement/src/bin/hnlc.rs`) and `host-lsp` (for
    refinement hover). The eval runtime (`handl-runtime`,
    `kit-canonical`, `kit-stdlib`, `host-cli`) has **zero dependency**
    on `plugin-ext-refinement` (confirmed: no Cargo.toml dep, no `use`
    statements).
- **Bug:** Even if the `import` Foreign op (9.5.1) were implemented,
  there's no code in the eval runtime to actually load a handl-source
  module. The `ModuleRegistry` only holds Rust-registered `Module`
  trait objects (`platform/src/eval.rs:374`) — host functions like
  `collections.push`, not handl-source files. There's no:
  - `load_module(path)` that reads a `.hnl` file, lexes/parses/evaluates
    it, and returns its exported bindings.
  - `resolve_module(name)` that maps a module name to a file path
    (relative to the importer, or a configured module root).
  - Cache of already-loaded modules (to avoid re-evaluating on
    re-import).
  - Cycle detection for circular imports in the eval path.
  The arrow-loader infrastructure in `plugin-ext-refinement` does all
  of this but for `Scheme`s, not ASTs, and it's not reachable from the
  eval runtime.
- **Fix:** Two strategies, depending on the 9.5.1 design decision:
  1. **If `import` is compile-time (desugar pass):** The desugar pass
     needs a module loader that works on ASTs. Either:
     - **Extract** the `ModuleResolver`/`ModuleLoader` pattern from
       `plugin-ext-refinement` into a shared crate (e.g.
       `plugin-lib-module-loader` or into `handl-runtime`), generalized
       to load handl-source ASTs instead of `Scheme`s. The resolver
       interface (`resolve(path) -> source text`) is reusable as-is;
       the loader changes (lex/parse instead of `parse_arrow_module`).
     - **Add** an AST-level `ModuleLoader` that caches parsed `Ast`s
       (not `Scheme`s), with cycle detection mirroring `load_transitive`.
     - The desugar pass calls the loader, gets the imported module's
       AST, extracts its exported bindings, and splices them into the
       importer's AST.
  2. **If `import` is runtime (Foreign op):** The `import` module
     function needs access to a loader. Either:
     - Same extraction as (1) — a shared loader crate the Foreign op
       calls.
     - Or give the `import` op access to the `Runtime` directly (it can
       call `runtime.parse()` + `runtime.eval()` on the loaded source).
       Less clean but simpler.
  3. **Reuse vs rewrite:** The `ModuleResolver` trait, `FileResolver`,
     `MemoryResolver`, `ChainResolver`, `CachingResolver` are
     generic enough to reuse — they resolve paths to source text, which
     is engine-agnostic. Only the `ModuleLoader` (which parses the
     source into `Scheme`s) needs an AST variant. Extract the resolvers
     into a shared crate, then add an AST-based loader alongside the
     existing Scheme-based one.
- **Sub-tasks:**
  1. Extract `ModuleResolver`/`FileResolver`/`MemoryResolver`/
     `ChainResolver`/`CachingResolver` from `plugin-ext-refinement` into
     a shared crate (or into `handl-runtime`).
  2. Add `AstModuleLoader` (or generalize `ModuleLoader`) that loads
     `.hnl` source → lex → parse → cache `Ast`, with cycle detection.
  3. Wire the loader into the eval runtime (either the `import` Foreign
     op or the desugar pass, per 9.5.1).
  4. Add a module-path resolution config (root directory, search paths)
     to `Runtime` or `Kit`.
- **Test:**
  - Two-file program: `main.hnl` imports `utils.hnl`, calls a function
    from it → evaluates correctly.
  - Three-file transitive import: `a` imports `b` imports `c` → `a`
    gets `c`'s exports transitively.
  - Re-import: `a` imports `b` twice (directly + transitively through
    `c`) → `b` is evaluated once (cache hit).
  - Circular: `a` imports `b`, `b` imports `a` → clear error, no hang.
  - Missing file → error with the path.
- **Files:**
  - `crates/plugin-ext-refinement/src/arrow_loader.rs` (extract
    resolvers).
  - New `crates/plugin-lib-module-loader/` or `crates/handl-runtime/src/
    module_loader.rs` (AST-based loader).
  - `crates/handl-runtime/src/lib.rs` (wire loader into runtime).
  - `crates/kit-canonical/src/lib.rs` (module-path config on `Kit`).
  - `crates/plugin-ext-refinement/Cargo.toml` (depend on the extracted
    crate instead of owning the resolvers).
- **Depends on:** 9.5.1 (the design decision determines whether the
  loader is called from a desugar pass or a Foreign op).

---

## Phase 10 — Pipeline & error infrastructure

### 10.1 — Typechecker not run before eval  (Pipeline gap)

- **Where:** `crates/handl-runtime/src/lib.rs:793-797` (`Runtime::run`)
  and `crates/kit-canonical/src/lib.rs:626` (`Kit::run` delegates to
  `Runtime::run`).
- **Bug:** `Runtime::run` is `lex → parse → eval` — no typecheck step.
  `Kit::run` (the public API callers use) delegates to it, so every
  `kit.run(input)` call evaluates without typechecking. Type errors
  (unbound variables, wrong arity, predicate failures) surface as
  runtime crashes or silent wrong results instead of compile-time
  diagnostics. `Kit::typecheck` exists (lib.rs:614) but is a separate
  entry point — `run` never calls it.
- **Fix options:**
  1. **Insert typecheck into `run` before eval.** `run` becomes
     `lex → parse → typecheck → eval`. Typecheck failures short-circuit
     (return `Err` before eval). This is the safest default — no
     silently-unsafe evaluation. But it changes `run`'s contract: code
     that currently "works" (runs despite type errors) would start
     failing. Audit all `run` callers first.
  2. **Add `run_checked` alongside `run`.** Keep `run` as-is (lex→parse→eval)
     for backward compat; add `run_checked` that typechecks first. Make
     `Kit::run` call `run_checked` by default, with an escape hatch for
     raw eval. Less disruptive but leaves a footgun.
  3. **Make typecheck optional via a flag/config.** `Runtime::run` checks
     a config field (`typecheck_before_eval: bool`); default true.
     Prefer option 1 or 2 — a flag is a config burden.
- **Decision factor:** Whether any existing caller *relies* on `run`
  evaluating despite type errors (e.g. REPL-style incremental input
  where partial type errors shouldn't block eval). If yes, option 2. If
  no, option 1.
- **Test:**
  - `kit.run("x : Int = \"hello\"")` → `Err` with a type mismatch
    diagnostic (not a runtime crash or silent wrong value).
  - `kit.run("1 + 2")` → `Ok(3)` (typechecks clean, then evals).
  - If option 2: `kit.run(input)` typechecks; a raw
    `kit.runtime().eval(&ast)` path skips typecheck.
- **Files:** `crates/handl-runtime/src/lib.rs` (`Runtime::run`),
  `crates/kit-canonical/src/lib.rs` (`Kit::run`), audit all `run`
  callers across `crates/`.

### 10.2 — Engine errors bypass span info  (Error quality gap)

- **Where:** `crates/handl-language/src/error.rs:66-73` —
  `LangError::Engine { message, span: Option<Span>, node_kind, context }`.
  The `span` field exists but engines almost never populate it.
- **Bug:** Three constructors exist:
  - `LangError::engine(msg)` — sets `span: None`. Used **~500 times**
    across the codebase.
  - `LangError::engine_node(msg, node)` — copies `node.span`. Used **284
    times** (better, but many of those are in compile paths, not eval).
  - `LangError::engine_at(msg, span, kind)` — explicit span. Used **1
    time** in the entire codebase.
  - `with_node` / `with_span` builder methods: **don't exist** on
    `LangError` (the `with_span` grep hits are on `AstNode`, a different
    type).
  - Per-engine `engine()` (no span) counts: tree-walk ~39, bytecode ~31,
    cranelift ~179. The vast majority of eval errors carry no span, so
    the user sees `"expected number, got ..."` with no source location.
- **Root cause:** Engines call `LangError::engine(format!(...))` because
  it's the path of least resistance. The node is often in scope (the
  eval scheme receives the `AstNode`), but the code uses `engine()` not
  `engine_node()`. There's no lint or CI gate catching this.
- **Fix:** Two layers:
  1. **Audit + convert high-value call sites.** Walk every
    `LangError::engine(` in `plugin-engine-eval-tree-walk`,
    `plugin-engine-eval-bytecode`, `plugin-engine-eval-cranelift`,
    `plugin-engine-parse`, `plugin-engine-lex`, and
    `plugin-engine-typecheck-*`. Where the `AstNode` is in scope, switch
    to `engine_node(msg, node)`. Where only a `Span` is available (e.g.
    lexer errors), use `engine_at(msg, Some(span), None)`. Prioritize
    eval + parse (user-facing) over compile paths.
  2. **Add a lint/test gate.** Add a `#[test]` or grep-based CI check
    that flags new `LangError::engine(` calls in engine crates (allow
    them only where no node/span is genuinely available — e.g. "unknown
    engine name" dispatch errors in `Runtime`). This prevents regression.
  3. **Consider adding `with_node`/`with_span` builders** on `LangError`
    for ergonomic chaining, if the audit reveals a common pattern of
    "create error, then attach span." But the constructors already
    cover this; builders may be unnecessary.
- **Test:**
  - Eval `"1 + true"` → error message includes a span (e.g. `at 0..5`
    or similar), not just `"expected number, got Bool(true)"`.
  - Parse `"let x = "` → parse error includes the EOF position.
  - The CI gate test fails if a new `engine()` call is added without
    justification.
- **Files:** All `crates/plugin-engine-*/src/` (audit + convert),
  `crates/handl-language/src/error.rs` (builders if added),
  CI config or a workspace-level test for the gate.
- **Scope note:** ~500 call sites is a large audit. Do it incrementally:
  one engine crate per PR, starting with tree-walk (most user-facing).
  Track remaining count in this file as it shrinks.

---

## Phase 11 — `host-cli` `--plugin` flag  (Bonus)  [~ — ADR-0090 rewrote host-cli with subcommand surface; `--plugin` flag still not implemented]

### 11.1 — `host-cli` doesn't implement `--plugin`  [~ — ADR-0090 rewrote `parse_args`; `--plugin` arm still missing. Decision: option 2 (mark test `#[ignore]`) since ADR-0090's `hnl` subcommand surface is the canonical CLI now]

- **Where:** `crates/host-cli/src/main.rs:466-509` (`parse_args` has no
  `--plugin` arm). Test `cli_loads_native_plugin_and_runs_echo`
  (`crates/host-cli/tests/integration.rs:39`) invokes `hnl --plugin
  <manifest> echo hello` but `--plugin` is silently treated as the
  command (since `command.is_empty()` on the first iteration), so the
  test fails instead of skipping.
- **Task:** Pick one:
  1. **Implement `--plugin`** in `parse_args`: add a `--plugin <path>`
     arm that loads a plugin manifest and registers it with the host
     before dispatching the command. This makes the integration test
     pass for real.
  2. **Mark the test `#[ignore]`** with a note matching the
     `host-tooling` one ("cli plugin wiring not yet implemented") to
     stop the bleeding. This is the minimal fix if `host-cli` is slated
     for deletion (TODO.md Phase 3 deletes `host-cli` once `hnl`
     reaches parity).
- **Decision factor:** If ADR-0085's `tooling-cli` (`hnl`) will replace
  `host-cli` soon, option 2 (ignore) is correct — don't invest in
  `host-cli`. If `host-cli` is needed *now* for plugin workflows,
  option 1.
- **Files:** `crates/host-cli/src/main.rs`,
  `crates/host-cli/tests/integration.rs`.

---

## Dependency graph

```
Phase 0 (data bugs) ──┬─→ Phase 1 (surface syntax) ──→ Phase 2 (recursor gen)
                      │                                   │
                      │                                   ├─→ Phase 6.1 (termination)
                      │                                   ├─→ Phase 6.2 (match sugar)
                      │                                   └─→ Phase 6.3/6.4 (families/dependent)
                      │
                      └─→ Phase 5 (runtime-assert) — independent of 1/2

Phase 3 (B2 gaps) — independent of 0/1/2; can run in parallel

Phase 4 (genericity) — defer until 0–3 stable (kind set still changing)

Phase 7.1 (Total) depends on Phase 6.1
Phase 7.2 (Pure) — independent

Phase 8 (FFI) — independent of 0–7; ADR-0083 track
Phase 9 (tooling) — independent; check ADR-0085/0086/0087 first
Phase 9.5 (import/export) — 9.5.2 (module loader) depends on 9.5.1
        (import op design decision); blocks multi-file programs
Phase 10 (pipeline/errors) — independent; 10.1 (typecheck-before-eval)
        should land before Phase 5 (runtime-assert) so the assert nodes
        are actually checked; 10.2 (span info) is independent
Phase 11 (host-cli) — independent; decide ignore vs implement
```

## Suggested ordering

1. **Phase 0** (blocks everything in the data track) — do first.
2. **Phase 1** (makes Phase 0 testable from source) — do alongside 0.
3. **Phase 3** (B2 gaps) — independent, can be parallelized with 0/1.
4. **Phase 2** (recursor generality) — after 0.
5. **Phase 10.1** (typecheck before eval) — before Phase 5; quick
   contract change, large safety win.
6. **Phase 5** (runtime-assert) — after 0 + 10.1; closes ADR-0068.
7. **Phase 11** (host-cli) — quick win, do anytime.
8. **Phase 6** (ADR-0082 steps 3+) — after 2.
9. **Phase 7** (Total/Pure) — after 6.1 (Total); Pure anytime.
10. **Phase 8** (FFI Cranelift + marshalability) — ADR-0083 track, larger.
11. **Phase 9** (tooling scaffolds) — check ADR-0085/0086/0087 first;
    may be superseded.
12. **Phase 9.5** (import/export) — blocks multi-file programs; check
    ADR-0077/0074 for compile-time vs runtime semantics first.
13. **Phase 10.2** (span info audit) — incremental, one crate per PR;
    can start anytime but low urgency.
14. **Phase 4** (genericity) — last; kind set must be stable.

---

## Quick fixes — do immediately

These are trivial, block CI, and don't depend on any phase.

### Q.1 — `handl-value` test build failure  (CI blocker)  [x — verified already fixed in tree; 44 tests pass]

- **Where:** `crates/handl-value/tests/resource.rs:91,98` — accesses
  `fr.dropped`, a private field on `ForeignResource`
  (`crates/handl-value/src/lib.rs:68` — `dropped: AtomicBool`, not
  `pub`).
- **Bug:** The test at line 91 reads `fr.dropped.load(...)` directly,
  but `dropped` is private. Compilation fails with
  `error[E0616]: field 'dropped' of struct 'ForeignResource' is
  private`. This makes `cargo test -p handl-value` fail, and
  `cargo test --workspace` is red.
- **Note:** The `basics.rs` test (the `HashMap` → `FxHashMap` migration
  the user mentioned) was **already fixed** — it uses
  `FxHashMap::default()` at line 294 and imports `FxHashMap` at line
  11. The remaining failure is in `resource.rs`, not `basics.rs`.
- **Fix:** The struct already has `is_dropped()` (used at line 88 and
  95 of the test). Replace the two direct `fr.dropped` accesses:
  - Line 91: `fr.dropped.load(std::sync::atomic::Ordering::SeqCst)`
    → `fr.is_dropped()` (or expose a `dropped_flag()` accessor if the
    test needs the raw `AtomicBool`).
  - Line 98: same.
  - If the test needs to *reset* the flag (line 103 comment says "Reset
    for cleanliness"), add a `#[cfg(test)]` reset method or make
    `dropped` `pub(crate)` with a test-only accessor.
- **Test:** `cargo test -p handl-value --no-run` compiles clean;
  `cargo test -p handl-value` passes.
- **Files:** `crates/handl-value/tests/resource.rs` (replace
  `fr.dropped` with `fr.is_dropped()` or accessor),
  `crates/handl-value/src/lib.rs` (add accessor if needed).

### Q.2 — Eval-engine matches missing `Bytes`/`Resource` arms  (CI blocker, found 2026-09-04)  [x — fixed 2026-09-04; all 3 engine crates compile, 254 lib tests pass]

- **Where:** `handl-plugin-engine-eval-tree-walk`, `handl-plugin-engine-eval-bytecode`,
  `handl-plugin-engine-eval-cranelift` — `error[E0004]`: non-exhaustive `match`
  on `&TaggedValue`, missing `Bytes(_)` and `Resource(_)` arms. Fallout from
  new `handl-value` variants added in the dirty working tree.
- **Fix:** Add the missing arms to each match (sensible behavior: error /
  "not supported in this engine" with `LangError::engine_node`, unless an
  engine genuinely can evaluate them).
- **Test:** `cargo test --workspace --no-run` compiles clean.
