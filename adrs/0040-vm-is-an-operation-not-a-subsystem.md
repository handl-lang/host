---
title: VM is an Operation, Not a Subsystem
type: adr
date: 2026-09-02
status: accepted
supersedes: [ADR-0027]
---

# VM is an Operation, Not a Subsystem

## Context

The platform's stated vision (see `docs/explanations/design-principles.md` and `docs/explanation/runtime.md`) is that **everything is a plugin**: syntax, semantics, tooling, targets, the garbage collector, the sandbox, and the standard library. A `Host` is an empty composer that loads kits and plugins and dispatches `CapabilityRequest`s. The kernel defines only the composition primitives (`Plugin`, `Host`, `Operation`, `Behavior`, `RuleAcceptor`, `Contribution`).

The platform already implements this vision for every front-end stage: `Lexer`, `Parser`, `Typecheck`, `Lint`, `Format`, and `Eval` are all `Operation`s served by `Basic*` `RuleAcceptor` plugins that accept `Behavior` rules. `plugin-typecheck-core` is the canonical `TypecheckBehavior` rule, contributed into `BasicTypechecker` via the kit. `plugin-eval-hello` and `plugin-eval-json` are existing `EvalBehavior` rules, contributed into `BasicEvaluator` — they decode `AstNode.payload` directly and return `serde_json::Value`, proving the eval-plugin pattern already works. The pattern is proven.

**Two subsystems have not been migrated to this pattern: the VM layer and the Runtime layer.** Together they define _three_ parallel trait hierarchies that duplicate the platform's `Operation`/`Behavior`/`RuleAcceptor`/`Plugin`/`Host` abstractions, and _two_ parallel function-registration mechanisms that duplicate the `Module`/`ModuleRuntime` pattern.

### The VM layer (5 crates)

`plugin-vm`, `plugin-vm-common`, `plugin-vm-backend-rust-interpreter`, `plugin-vm-host-eval`, `plugin-vm-values-json` define a parallel trait hierarchy (`VirtualMachine<V>`, `Host<V>`, `Function<V>`, `Operation<V>`, `Value`, `Context<V>`) that duplicates the platform's abstractions. Hosts link these crates directly as Rust dependencies and instantiate `CanonicalVm::new(host)` by hand. `host-cli`'s `run` and `eval` commands literally comment "bypassing the plugin system."

### The Runtime layer (6 crates)

`plugin-runtime`, `plugin-runtime-apply`, `plugin-runtime-arithmetic`, `plugin-runtime-array`, `plugin-runtime-boolean`, `plugin-runtime-object` define a _second_ function-registration mechanism (`RuntimeBehavior` → `RuntimeHost` `RuleAcceptor`) that duplicates `Module`/`ModuleRuntime`. Worse, they re-implement functions that already exist in `plugin-lib-utils` and `plugin-lib-collections`:

| `plugin-runtime-*` function | Already in                                                 |
| --------------------------- | ---------------------------------------------------------- |
| `add`/`sub`/`mul`/`div`     | `plugin-lib-utils` (`math.*`)                              |
| `and`/`or`/`not`/`eq`       | `plugin-lib-utils` (`bool.*`, `cmp.*`)                     |
| `lt`                        | `plugin-lib-utils` (`cmp.lt`)                              |
| `fn.pipe`                   | `plugin-lib-utils` (`fn.pipe`)                             |
| `array.at`/`array.length`   | `plugin-lib-collections` (`array.at`, `array.len`)         |
| `obj.get`/`obj.keys`        | `plugin-lib-collections` (`hash_map.get`, `hash_map.keys`) |
| `apply` (eval primitive)    | `plugin-vm-host-eval` — belongs in `plugin-eval-core`      |

ADR 0027 proposed entrenching the VM parallel hierarchy: keeping `handl-vm` as a generic abstraction crate, keeping `handl-plugin-vm-values-json` as one `Value` implementation, and adding `handl-plugin-vm-backend-bytecode` / `handl-plugin-vm-backend-jit` as peers. That direction doubles down on the indirection rather than removing it. This ADR reverses that decision and also removes the Runtime layer's duplication.

### Why both layers are unnecessary indirection

1. **Duplicate abstraction.** `plugin-vm`'s `VirtualMachine<V>` is the platform's `BasicEvaluator` + `EvalBehavior` re-expressed. `plugin-vm`'s `Host<V>` is the platform's `Host` re-expressed. `plugin-vm`'s `Operation<V>` is the platform's `Operation` marker (name collision, different concept). `plugin-runtime`'s `RuntimeBehavior`/`RuntimeHost` is `Module`/`ModuleRuntime` re-expressed. Three trait hierarchies for two ideas ("pluggable composed thing" and "register functions") is indirection with no consumer that benefits from the split.

2. **Unused genericity.** `plugin-vm` is generic over `V: Value`. The docs commit to `serde_json::Value` as the value representation. Only one `V` exists (`NonFunctionValue`, a newtype over `serde_json::Value`). Parameterizing the entire VM over `V` is speculative generality with zero users. ADR 0027's planned `handl-plugin-vm-values-*` peer crates never materialized.

3. **Inconsistent with the rest of the platform.** No other operation has a parallel trait hierarchy. `Typecheck` does not have a `Typechecker<V>` trait in a separate crate; it has `BasicTypechecker` (a `RuleAcceptor` plugin) and `TypecheckBehavior` (a `Behavior` rule). The VM and Runtime layers' existence implies evaluation and function registration are special. They are not.

4. **Confusing taxonomy.** The five `plugin-vm-*` crates mix contracts, plumbing, a backend, a host bridge, and a value representation under one `plugin-vm-*` prefix, none of which implement `Plugin`. The prefix implies they are plugins; they are not. `ModuleRuntime` inside `plugin-vm-common` _is_ a plugin, which makes the split incoherent. The six `plugin-runtime-*` crates are plugins but duplicate `plugin-lib-*` functions under a different registration mechanism.

5. **Blocks the documented vision.** Design principle #14 ("Evaluation is an operation, not a subsystem") and #17 ("Targets are plugins") cannot be satisfied while the VM layer exists as a peer abstraction. GC-as-plugin (#15) and sandbox-as-kit-property (#16) also require the execution backend to be a selectable target plugin, not a directly-linked crate.

## Decision

Dissolve both the `plugin-vm-*` and `plugin-runtime-*` crate families. Move evaluation onto the platform's `Eval` operation / `EvalBehavior` / `BasicEvaluator` pattern, identical to every other operation. The interpreter's per-AST-kind handlers become granular `EvalBehavior` plugins, mirroring the `plugin-grammar-*` pattern — each receives an `AstNode` and decodes its `payload`, returning `serde_json::Value`, exactly as `plugin-eval-hello` and `plugin-eval-json` already do. Function registration collapses to the single `Module`/`ModuleRuntime` mechanism. Values are `serde_json::Value`. There is no `VirtualMachine` trait, no `Value` trait, no `Runtime` operation, no `V` genericity, no `Expr`/`Pattern`/`HandlerClause` intermediate representation.

### Design details

#### Uniform `Behavior<Eval>` with context in request

Eval uses the same `Behavior<Eval>` trait as every other operation — no custom `EvalBehavior` signature. The difference is that `EvalRequest` carries an `EvalContext`:

```rust
// platform (extended)
pub struct EvalRequest {
    pub ast: AstNode,
    pub ctx: EvalContext,        // scopes, evaluator handle, module registry, limits
    pub options: ResolvedOptions,
    pub result: Option<Value>,
}
```

`Behavior<Eval>::apply(&self, req: &mut EvalRequest)` stays the same signature. Handler accesses `req.ctx.eval(&child)` to recurse. Consistent with typecheck/lint/format — eval isn't special at the trait level, just at the request level.

#### `EvalContext`

```rust
// plugin-ext-eval
pub struct EvalContext {
    scopes: Vec<Scope>,              // variable bindings (JSON values)
    evaluator: EvaluatorHandle,      // callback into BasicEvaluator for recursion
    modules: ModuleRegistry,         // function lookup
    limits: EvalLimits,              // max_depth, max_calls
    call_count: usize,
}

impl EvalContext {
    pub fn eval(&mut self, node: &AstNode) -> Result<Value, EvalError> {
        self.evaluator.dispatch(node, self)
    }
    pub fn eval_async(&mut self, node: &AstNode) -> impl Future<Output = Result<Value, EvalError>> { ... }
    pub fn call_function(&mut self, name: &str, args: &[Value]) -> Result<Value, EvalError> { ... }
}
```

`EvalContext` lives in `plugin-ext-eval` and is embedded in `EvalRequest` (defined in `platform`).

#### Recursive evaluation

`Behavior<Eval>` rules are not leaf-only — `plugin-eval-call` must evaluate arguments, `plugin-eval-apply` must evaluate a function body, `plugin-eval-handle` must evaluate handler arms. To recurse, a handler calls `req.ctx.eval(&child_node)`, which dispatches through `EvaluatorHandle` back into `BasicEvaluator`, which finds the matching `Behavior<Eval>` rule for the child's `AstNode.kind`. Standard interpreter pattern — the context carries the eval function.

Child nodes are embedded as JSON objects inside the parent's `AstNode.payload`. To recurse, a handler extracts a JSON object, reconstructs an `AstNode { kind, payload }`, and calls `ctx.eval(&child)`. This is the JSON-boundary consequence of design principle #6.

#### `Function` trait replacement

`Module::function()` currently returns `&dyn Function<V>`. With `V` deleted, a non-generic `Function` trait lives in `plugin-module`:

```rust
// plugin-module
pub trait Function: Send + Sync {
    fn call(&self, args: &[serde_json::Value], ctx: &mut EvalContext) -> Result<serde_json::Value, EvalError>;
}

pub trait Module: Send + Sync {
    fn name(&self) -> &str;
    fn function(&self, name: &str) -> Option<&dyn Function>;
    // ...
}
```

`EvalContext` is defined in `plugin-ext-eval` and passed through. No genericity anywhere.

#### Function values as JSON

First-class functions are represented as JSON objects with an implicit schema, documented in `plugin-eval-core`:

```json
{ "params": [["x", null], ["y", null]], "body": <ast payload>, "captured": { "z": 42 } }
```

`apply` reads this schema, binds params, merges captured vars, and calls `ctx.eval(&body)`. No type safety — that's the JSON-boundary tradeoff the platform already commits to (design principle #6, #8). This convention is stated explicitly so nobody expects typed function values later.

#### Async evaluation

`EvalContext` provides two eval paths: `eval` (sync) and `eval_async` (returns a `Future`). `Behavior<Eval>` rules that need async (e.g. calling IO-bound host functions) implement the async path; sync rules use the sync path. `BasicEvaluator` dispatches based on which method was called. The platform's `Plugin::run` contract stays sync — async happens _inside_ evaluation, not at the capability dispatch boundary.

#### Exec-control as lightweight hooks

Exec-control (limits, cache, trace, etc.) uses a minimal two-hook trait, not a full middleware wrapping trait. No other operation has middleware; eval is the only op with cross-cutting execution concerns, so this one deviation is justified:

```rust
// plugin-ext-eval
pub trait ExecControl: Send + Sync + 'static {
    fn before_eval(&self, ast: &AstNode, ctx: &mut EvalContext) -> ControlFlow<()>;
    fn after_eval(&self, ast: &AstNode, result: &Value, ctx: &mut EvalContext) {}
}

pub struct BasicExecControl {
    rules: Vec<Box<dyn ExecControl>>,
}
```

`BasicEvaluator` calls all `ExecControl` rules before/after each dispatch. Kit-selectable, composable, but minimal — two hooks, not a wrapping trait. `plugin-exec-*` plugins implement `ExecControl`. A sandbox kit loads `plugin-exec-deny` + `plugin-exec-max-depth` + `plugin-exec-max-calls`; their `before_eval` checks gate execution. No wrapping overhead when no exec-control rules are loaded.

#### Continuation / yield / resumability

The context _is_ the continuation. `EvalContext` is serializable (it's JSON values + a scope stack of JSON). Yielding replaces the current `Error::Yield` + `Continuation` pair with:

```rust
pub enum EvalError {
    Yield { context: EvalContext },
    // ...
}
```

`plugin-exec-cache` captures the yielded context, serializes it, and on resume re-dispatches `Eval` with a restored `EvalContext`. Cleaner than the current `Continuation` struct — there's no separate continuation type, the context state is all you need.

#### Dispatch performance

`BasicEvaluator` keys rules by `AstNode.kind` in a `HashMap` at registration time — O(1) lookup per node, not a linear scan. `ExecControl` rules are called before/after dispatch; when none are loaded, the overhead is a zero-length slice iteration. For a tree-walking interpreter this is fine. Performance-critical paths use the `Compile` operation (compilation eliminates per-node dispatch entirely). Acknowledged tradeoff: the plugin model trades a direct enum match for a hash lookup per node, in exchange for composability and replaceability.

#### Test migration

- `plugin-vm-backend-rust-interpreter`'s `vm_tests.rs`, `handler_tests.rs`, `continuation_tests.rs`, `operation_tests.rs`, `generic_tests.rs` → ported into the respective `plugin-eval-*` crates.
- `plugin-vm-common`'s tests → ported into `plugin-module`, `plugin-exec-*`, and `handl-test-utils`.
- `plugin-vm-host-eval`'s tests → ported into `plugin-eval-core`.
- Continuation tests adapted to the `EvalError::Yield { context }` model.

### Crate mapping

#### VM layer

| Current crate                                                                                                                               | Disposition                                                                                                                                                                                                                                                                                                                                                                 | Target                                                     |
| ------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| `plugin-vm` (traits: `VirtualMachine`, `Host<V>`, `Function<V>`, `Operation<V>`, `Value`, `Context<V>`, `Expr`, `Pattern`, `HandlerClause`) | **Delete.** All trait duplicates of platform abstractions (`Host`/`Operation`/`Plugin`) are dropped — use the platform's. `Expr`/`Pattern`/`HandlerClause` are an unnecessary intermediate representation: eval rules receive `AstNode` and decode its `payload` directly, as `plugin-eval-hello`/`plugin-eval-json` already do. No replacement.                            | Deleted                                                    |
| `plugin-vm-values-json` (`NonFunctionValue`, `FunctionValue`, `CanonicalContext`)                                                           | **Delete.** Values are `serde_json::Value` per the docs. `NonFunctionValue` newtype is dropped — use `serde_json::Value` directly. `CanonicalContext` moves into `plugin-ext-eval` as the eval context. No `Value` trait.                                                                                                                                                   | `plugin-ext-eval` (context) / `serde_json::Value` (values) |
| `plugin-vm-backend-rust-interpreter` (`CanonicalVm`, 11 `*Handler` structs)                                                                 | **Split into 11 granular plugins** (one `EvalBehavior` rule per AST kind), mirroring the `plugin-grammar-*` pattern. No `plugin-target-rust-interpreter` crate — the "target" is emergent from which eval plugins the kit loads.                                                                                                                                            | 11 `plugin-eval-*` plugins (see below)                     |
| `plugin-vm-host-eval` (`EvalHost`, `Var`/`Resolve`/`Apply`)                                                                                 | **Delete.** `EvalHost` is an artifact of the VM having its own `Host<V>` trait separate from the platform's `Host` — it dissolves entirely. The built-in `var`/`resolve`/`apply` functions are the eval kernel's minimum primitives (bind lookup, path lookup, function application); they move into `plugin-eval-core` as `EvalBehavior` rules, part of the eval contract. | `plugin-eval-core`                                         |
| `plugin-vm-common`                                                                                                                          | **Split** (see below).                                                                                                                                                                                                                                                                                                                                                      | multiple                                                   |

#### `plugin-vm-common` split

| Component                                                                                                        | Disposition                                                                                                                                                                                  | Target                                                                            |
| ---------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| `host/host.rs` (`ModuleRuntime`, `ModuleRule`)                                                                   | Already a `Plugin` + `RuleAcceptor`. Stays as a plugin.                                                                                                                                      | `plugin-module`                                                                   |
| `module/module.rs` (`Module` trait)                                                                              | Contract for named function namespaces.                                                                                                                                                      | `plugin-module`                                                                   |
| `module/module_stub.rs`                                                                                          | Test stub.                                                                                                                                                                                   | `handl-test-utils`                                                                |
| `host/dynamic_runtime.rs` (`DynamicRuntime`)                                                                     | Host wiring.                                                                                                                                                                                 | `host-cli`/`host-lsp` host code                                                   |
| `capabilities/cache/` (`CallCache`, `CacheHost`)                                                                 | Execution control: skip re-execution, return cached result (resumability).                                                                                                                   | `plugin-exec-cache`                                                               |
| `adapter.rs` (`Memoize`, `Pipe`, `Catch`, `Partial`, `Timed`, `If`, `Const`, `Identity`, `MapArgs`, `MapResult`) | Rust-side function combinators (wrap a `Function` impl). Dev plumbing, not user-facing.                                                                                                      | `plugin-ext-eval` (fn-registration adapters)                                      |
| `combinator.rs` (`ChainHost`, `FilterHost`, `SpyHost`)                                                           | Host combinators.                                                                                                                                                                            | `host-cli`/`host-lsp` host code                                                   |
| `middleware.rs` (`MapHost`, `LoggingHost`, `AllowHost`, `DenyHost`)                                              | Execution control rules.                                                                                                                                                                     | `plugin-exec-map`, `plugin-exec-logging`, `plugin-exec-allow`, `plugin-exec-deny` |
| `middleware_vm.rs` (`MiddlewareVm`, `VmMiddleware`, `Trace`, `Counter`, `NoopMiddleware`, `Chain`)               | Execution control rules. `ExecControl` trait + `BasicExecControl` → `plugin-ext-eval`.                                                                                                       | `plugin-ext-eval` (contract), `plugin-exec-trace`, `plugin-exec-counter`          |
| `machine.rs` (`ContextBuilder`, `Machine`)                                                                       | Eval context builder.                                                                                                                                                                        | `plugin-ext-eval`                                                                 |
| `lego.rs` (`ExprHandler` trait, `LegoVm`)                                                                        | Was the granular-handler pattern. Now superseded by `EvalBehavior` rules that decode `AstNode` directly. `ExprHandler` and `LegoVm` are deleted — `BasicEvaluator` already serves this role. | Deleted                                                                           |
| `sandbox.rs` (`with_max_depth`, `with_max_calls`)                                                                | Execution control: resource limits.                                                                                                                                                          | `plugin-exec-max-depth`, `plugin-exec-max-calls`                                  |
| `builder.rs` (`FnFunction`, `FnOperation`, `FnLazy`, `FnGeneric`, `FnAsync`)                                     | Closure → `Function`/`Operation` adapters.                                                                                                                                                   | `plugin-ext-eval`                                                                 |
| `mock.rs` (`NoopHost`, `MockHost`, `CaptureFn`)                                                                  | Test mocks.                                                                                                                                                                                  | `handl-test-utils`                                                                |
| `prelude.rs`                                                                                                     | Re-exports.                                                                                                                                                                                  | Deleted; each plugin exports its own prelude.                                     |

#### Runtime layer

| Current crate                                                                    | Disposition | Reason                                                                                        |
| -------------------------------------------------------------------------------- | ----------- | --------------------------------------------------------------------------------------------- |
| `plugin-runtime` (`Runtime` op, `RuntimeBehavior`, `RuntimeHost`, `RuntimeRule`) | **Delete.** | Duplicate of `Module`/`ModuleRuntime`. The `Runtime` operation in `platform` is also deleted. |
| `plugin-runtime-arithmetic` (`add`/`sub`/`mul`/`div`)                            | **Delete.** | Already in `plugin-lib-utils` (`math.*`).                                                     |
| `plugin-runtime-boolean` (`and`/`or`/`not`/`eq`/`lt`)                            | **Delete.** | Already in `plugin-lib-utils` (`bool.*`, `cmp.*`).                                            |
| `plugin-runtime-array` (`array.at`/`array.length`)                               | **Delete.** | Already in `plugin-lib-collections` (`array.at`, `array.len`).                                |
| `plugin-runtime-object` (`obj.get`/`obj.keys`)                                   | **Delete.** | Already in `plugin-lib-collections` (`hash_map.get`, `hash_map.keys`).                        |
| `plugin-runtime-apply` (`apply`/`fn.pipe`)                                       | **Delete.** | `fn.pipe` already in `plugin-lib-utils`. `apply` (eval primitive) → `plugin-eval-core`.       |

### Resulting taxonomy

#### Eval plugins (new/extended)

- **`plugin-ext-eval`** — already exists as a thin re-export of `Eval`/`EvalBehavior`/`BasicEvaluator`/`EvalRequest` from `platform`, mirroring `plugin-ext-typecheck`. **Extended** to also house `EvalContext` (from `CanonicalContext`), context builder, fn-registration adapters, and the `ExecControl` trait + `BasicExecControl` (the exec-control contract). Contract only.
- **`plugin-eval-core`** — builtins (`var`/`resolve`/`apply`) as `EvalBehavior` rules. The eval kernel's minimum primitives. Mirrors `plugin-typecheck-core`.
- **`plugin-eval-hello`** — already exists. `EvalBehavior` rule for `hello.greet`. Reference implementation of the eval-plugin pattern alongside `plugin-eval-json`.
- **`plugin-eval-json`** — already exists. `EvalBehavior` rule for JSON literal parsing. Reference implementation alongside `plugin-eval-hello`.
- **`plugin-eval-value`** — `EvalBehavior` rule for value literals. (From `ValueHandler`.)
- **`plugin-eval-call`** — `EvalBehavior` rule for call expressions. (From `CallHandler`.)
- **`plugin-eval-apply`** — `EvalBehavior` rule for apply expressions. (From `ApplyHandler`.)
- **`plugin-eval-array`** — `EvalBehavior` rule for array literals. (From `ArrayHandler`.)
- **`plugin-eval-object`** — `EvalBehavior` rule for object literals. (From `ObjectHandler`.)
- **`plugin-eval-index`** — `EvalBehavior` rule for index access. (From `IndexHandler`.)
- **`plugin-eval-field-access`** — `EvalBehavior` rule for field access. (From `FieldAccessHandler`.)
- **`plugin-eval-handle`** — `EvalBehavior` rule for handle expressions. (From `HandleHandler`.)
- **`plugin-eval-var`** — `EvalBehavior` rule for variable references. (From `VarHandler`.)
- **`plugin-eval-assign`** — `EvalBehavior` rule for assignments. (From `AssignHandler`.)
- **`plugin-eval-function`** — `EvalBehavior` rule for function literals. (From `FunctionHandler`.)

Note: the grammar also produces `pipe` and `match` AST kinds. `pipe` desugars to `apply` at the grammar layer (no separate eval handler needed). `match` is not yet implemented in the interpreter; when added, it will be a `plugin-eval-match` plugin following the same pattern. The 11 handlers above cover the currently implemented interpreter surface.

#### Module plugin (new/folded)

- **`plugin-module`** — `Module` trait + `ModuleRuntime` plugin + `ModuleRule`. The single function-registration mechanism. Every `plugin-lib-*` crate implements `Module` and registers here.

#### Exec-control plugins (new)

Execution control is composable: each rule is its own plugin, mirroring the `plugin-eval-*` and `plugin-grammar-*` granularity. The `ExecControl` trait + `BasicExecControl` contract lives in `plugin-ext-eval`; each rule is a separate crate implementing `ExecControl`. A kit selects which exec-control rules to load — a sandbox kit loads `plugin-exec-deny` + `plugin-exec-max-depth` + `plugin-exec-max-calls`; a debug kit loads `plugin-exec-trace` + `plugin-exec-counter` + `plugin-exec-logging`; a resumable kit loads `plugin-exec-cache`.

- **`plugin-exec-cache`** — skip re-execution, return cached result (cross-machine resumability). (From `CallCache`/`CacheHost`.)
- **`plugin-exec-trace`** — observe execution. (From `Trace`.)
- **`plugin-exec-counter`** — count execution. (From `Counter`.)
- **`plugin-exec-map`** — rewrite function names. (From `MapHost`.)
- **`plugin-exec-logging`** — log calls. (From `LoggingHost`.)
- **`plugin-exec-allow`** — allowlist functions. (From `AllowHost`.)
- **`plugin-exec-deny`** — blocklist functions. (From `DenyHost`.)
- **`plugin-exec-max-depth`** — bound recursion. (From `with_max_depth`.)
- **`plugin-exec-max-calls`** — bound total calls. (From `with_max_calls`.)

#### Test utilities (new)

- **`handl-test-utils`** — mocks (`NoopHost`, `MockHost`, `CaptureFn`), stubs (`StubModule`).

#### Unchanged

- **`plugin-typecheck-core`** — already a plugin. Reference implementation of the pattern this ADR applies to the VM.
- **`plugin-lib-utils`** — already has `math.*`, `bool.*`, `cmp.*`, `fn.*`. Unchanged.
- **`plugin-lib-collections`** — already has `array.at`, `array.len`, `hash_map.get`, `hash_map.keys`. **One change:** remove the `array.length` alias (keep `array.len` only).
- **Hosts** (`host-cli`, `host-lsp`, `kit-server`, etc.) — load eval plugins through the kit, like every other plugin. No direct `use handl_plugin_vm_backend_rust_interpreter::CanonicalVm;` import. The "bypassing the plugin system" comment in `host-cli` is deleted.

### What is explicitly NOT kept

- No `VirtualMachine<V>` trait.
- No `Value` trait. Values are `serde_json::Value`.
- No `V: Value` genericity anywhere in the eval path.
- No `handl-vm` abstraction crate.
- No `handl-plugin-vm-*` namespace.
- No `Runtime` operation, `RuntimeBehavior`, `RuntimeHost`, or `RuntimeRule`.
- No `plugin-runtime-*` crates (the 6 that duplicate `plugin-lib-*`).
- No `plugin-target-rust-interpreter` crate — the interpreter is 11 granular `plugin-eval-*` plugins, not a bundled target. The "target" is emergent from which eval plugins the kit loads.
- The 11 `plugin-eval-*` plugins are the **tree-walking interpreter target**. A compiler target (wasm/JIT/bytecode) does _not_ replace them with a different set of eval rules — it uses the `Compile` operation + `CompileBehavior` rules instead, since compilation processes the whole AST rather than evaluating per-node. "Targets are plugins" holds in both cases: an interpreter target is a set of `Eval` rules; a compiler target is a set of `Compile` rules. No new abstraction layer for either.

## Consequences

- **ADR 0027 is superseded.** Its crate layout, genericity-over-`V` design, and `handl-plugin-vm-*` namespace are abandoned.
- **`plugin-vm` is deleted.** Its trait surface is dropped entirely — platform traits replace the duplicates, and `Expr`/`Pattern`/`HandlerClause` are removed as unnecessary intermediate representation (eval rules decode `AstNode` directly).
- **`plugin-vm-values-json` is deleted.** Values become `serde_json::Value`; the eval context moves to `plugin-ext-eval`.
- **`plugin-vm-backend-rust-interpreter` is split into 11 `plugin-eval-*` plugins**, each contributing one `EvalBehavior` rule.
- **`plugin-vm-host-eval` is deleted.** Builtins move to `plugin-eval-core`; `EvalHost` dissolves.
- **`plugin-vm-common` is split** into `plugin-module`, `plugin-ext-eval` (adapters/builder/exec-control-contract), 9 `plugin-exec-*` plugins, and `handl-test-utils`. `lego.rs` (`ExprHandler`/`LegoVm`) is deleted — superseded by `Behavior<Eval>` + `BasicEvaluator`.
- **All 6 `plugin-runtime-*` crates are deleted.** Their functions are already in `plugin-lib-utils` and `plugin-lib-collections`. The `Runtime` operation in `platform` is deleted.
- **`plugin-lib-collections`** removes the `array.length` alias (keeps `array.len`).
- **`kit-canonical`** swaps `register_plugin(*Plugin::new())` for the `plugin-runtime-*` crates to contributing `MathModule`/`BoolModule`/`CmpModule`/`FnModule`/`CollectionsModule` via `plugin-module`, and loads the 11 `plugin-eval-*` plugins + `plugin-eval-core` + selected `plugin-exec-*` plugins through the kit.
- **~40 `plugin-lib-*` crates and 5 host/kit crates** that currently depend on `handl-plugin-vm*` must be migrated to depend on `plugin-ext-eval` + `serde_json::Value` instead. This is the bulk of the migration work.
- **`host-cli`'s `run`/`eval` commands** stop bypassing the plugin system. They dispatch `Eval` `CapabilityRequest`s through the host like every other operation.
- **Future wasm/JIT/bytecode targets** use the `Compile` operation + `CompileBehavior` rules (compilation processes the whole AST), not a parallel set of `Eval` rules. The 11 `plugin-eval-*` plugins are the tree-walking interpreter target; a compiler target is a different operation, not a different eval plugin set. No new abstraction layer in either case.
- **Future value representations** (if ever needed) would require revisiting the JSON commitment in `runtime.md`. This ADR does not introduce that option; it removes the unused one.
- **GC plugins** can be contributed by target or library plugins and selected by the host, satisfying design principle #15.
- **Sandboxing** becomes purely a kit/plugin composition property via `plugin-exec-deny` + `plugin-exec-max-depth` + `plugin-exec-max-calls`, satisfying design principle #16.

## Migration order

Executed sequentially, one subagent per step, multiple commits, no branch changes or PRs.

1. **`plugin-lib-collections` cleanup** — remove `array.length` alias (keep `array.len` only). Confirm `hash_map.get`/`hash_map.keys` cover `plugin-runtime-object`'s `obj.get`/`obj.keys`.
2. **Update `docs/explanation/runtime.md`** — remove the reference to `handl-target-rust-interpreter` as a named crate. Replace with: the interpreter target is emergent from which `plugin-eval-*` plugins the kit loads; a compiler target uses the `Compile` operation. Aligns the doc with this ADR.
3. **Extend `plugin-ext-eval`** — already exists as a thin re-export. Add: `EvalContext` (from `CanonicalContext`), context builder, fn-registration adapters, `ExecControl` trait + `BasicExecControl` (the exec-control contract). Also extend `EvalRequest` in `platform` to carry `EvalContext`.
4. **Land `plugin-eval-core`** — `var`/`resolve`/`apply` as `EvalBehavior` rules.
5. **Land the 11 `plugin-eval-*` handler plugins** — one `EvalBehavior` rule each, ported from `plugin-vm-backend-rust-interpreter`'s 11 handlers. Each receives `AstNode` and decodes its `payload`, returning `serde_json::Value`, following the `plugin-eval-hello`/`plugin-eval-json` pattern.
6. **Land `plugin-module`** — `Module` trait + `ModuleRuntime` plugin + `ModuleRule` (from `plugin-vm-common`).
7. **Land the 9 `plugin-exec-*` plugins** — `plugin-exec-cache`, `plugin-exec-trace`, `plugin-exec-counter`, `plugin-exec-map`, `plugin-exec-logging`, `plugin-exec-allow`, `plugin-exec-deny`, `plugin-exec-max-depth`, `plugin-exec-max-calls` (from `plugin-vm-common`'s cache/middleware/sandbox).
8. **Land `handl-test-utils`** — mocks, stubs.
9. **Migrate `kit-canonical`** — swap `register_plugin(*Plugin::new())` for `plugin-runtime-*` to contributing lib modules via `plugin-module`; load `plugin-eval-*` + `plugin-eval-core` + selected `plugin-exec-*` plugins through the kit.
10. **Migrate `host-cli`, `kit-server`, `kit-cli`, `kit-config`, `host-tooling`** — load eval through the kit instead of linking `CanonicalVm` directly. Delete the "bypassing the plugin system" code path.
11. **Migrate the ~40 `plugin-lib-*` crates** — from `handl-plugin-vm*` deps to `plugin-ext-eval` + `serde_json::Value`.
12. **Delete `plugin-vm`, `plugin-vm-values-json`, `plugin-vm-backend-rust-interpreter`, `plugin-vm-host-eval`.** Fold `plugin-vm-common`'s remaining bits into their target crates. Delete `lego.rs` (`ExprHandler`/`LegoVm`) — superseded by `EvalBehavior`.
13. **Delete `plugin-runtime`, `plugin-runtime-apply`, `plugin-runtime-arithmetic`, `plugin-runtime-array`, `plugin-runtime-boolean`, `plugin-runtime-object`** + the `Runtime` operation in `platform`.
14. **Update `Cargo.toml`** workspace members and dependency table.
15. **Mark ADR 0027 as superseded** by this ADR (already done in 0027's status).
