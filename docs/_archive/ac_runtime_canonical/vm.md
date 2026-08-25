**ac_runtime_canonical > vm**

# Module: vm

## Contents

**Structs**

- [`CanonicalVm`](#canonicalvm) - The language virtual machine.

**Functions**

- [`json_kind`](#json_kind)
- [`qualified_name`](#qualified_name)

**Type Aliases**

- [`RuntimeExpr`](#runtimeexpr)

---

## ac_runtime_canonical::vm::CanonicalVm

*Struct*

The language virtual machine.

The VM evaluates expressions by walking the expression tree.
Values are returned as-is, calls are dispatched to host-provided
functions via the [`Host`] trait.

# Type parameters

- `H` — the host type, which must implement [`Host`].

**Generic Parameters:**
- H

**Fields:**
- `host: H`

**Methods:**

- `fn new(host: H) -> Self` - Creates a new VM with the given host.
- `fn host(self: &Self) -> &H` - Returns a reference to the host.
- `fn eval(self: &Self, expr: &crate::Expr<ac_runtime_values::NonFunctionValue>, ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>` - Evaluates an expression in the given context.
- `fn eval_async(self: &Self, expr: &crate::Expr<ac_runtime_values::NonFunctionValue>, ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>` - Evaluates an expression asynchronously.
- `fn eval_resumable(self: &Self, expr: &crate::Expr<ac_runtime_values::NonFunctionValue>, ctx: & mut dyn Context, resume: Option<Continuation>) -> EvalResult` - Evaluates an expression using an explicit frame stack.
- `fn resume(self: &Self, cont: Continuation, ctx: & mut dyn Context) -> EvalResult`
- `fn unwind(self: &Self, frames: Vec<Frame>, value: NonFunctionValue, ctx: & mut dyn Context) -> EvalResult`
- `fn eval_frame(self: &Self, frame: & mut Frame, ctx: & mut dyn Context, stack: & mut Vec<Frame>) -> StepResult`
- `fn eval_step(self: &Self, expr: &crate::Expr<ac_runtime_values::NonFunctionValue>, ctx: & mut dyn Context, stack: & mut Vec<Frame>) -> StepResult`
- `fn json_kind(v: &JsonValue) -> &'static str`
- `fn eval_index(self: &Self, target: &crate::Expr<ac_runtime_values::NonFunctionValue>, index: &crate::Expr<ac_runtime_values::NonFunctionValue>, ctx: & mut dyn Context, stack: & mut Vec<Frame>, handlers: &[crate::HandlerClause<ac_runtime_values::NonFunctionValue>]) -> StepResult`
- `fn eval_field(self: &Self, target: &crate::Expr<ac_runtime_values::NonFunctionValue>, field: &str, ctx: & mut dyn Context, stack: & mut Vec<Frame>, handlers: &[crate::HandlerClause<ac_runtime_values::NonFunctionValue>]) -> StepResult`
- `fn eval_step_with_handlers(self: &Self, expr: &crate::Expr<ac_runtime_values::NonFunctionValue>, ctx: & mut dyn Context, stack: & mut Vec<Frame>, inherited_handlers: &[crate::HandlerClause<ac_runtime_values::NonFunctionValue>]) -> StepResult`
- `fn eval_handle(self: &Self, body: &crate::Expr<ac_runtime_values::NonFunctionValue>, handlers: &[crate::HandlerClause<ac_runtime_values::NonFunctionValue>], ctx: & mut dyn Context, stack: & mut Vec<Frame>) -> StepResult` - Evaluate a handle body with handler clauses active.
- `fn dispatch_call(self: &Self, name: &str, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>`
- `fn handle_operation(self: &Self, _name: String, args: Vec<NonFunctionValue>, clause: &crate::HandlerClause<ac_runtime_values::NonFunctionValue>, all_handlers: &[crate::HandlerClause<ac_runtime_values::NonFunctionValue>], ctx: & mut dyn Context, stack: & mut Vec<Frame>) -> StepResult` - Handle an operation signal by matching it against handler clauses

**Trait Implementations:**

- **VirtualMachine**
  - `fn eval(self: &Self, expr: &crate::Expr<ac_runtime_values::NonFunctionValue>, ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>`
  - `fn eval_async(self: &Self, expr: &crate::Expr<ac_runtime_values::NonFunctionValue>, ctx: & mut dyn Context) -> impl Trait`



## ac_runtime_canonical::vm::RuntimeExpr

*Type Alias*: `crate::Expr<ac_runtime_values::NonFunctionValue>`



## ac_runtime_canonical::vm::json_kind

*Function*

```rust
fn json_kind(v: &serde_json::Value) -> &'static str
```



## ac_runtime_canonical::vm::qualified_name

*Function*

```rust
fn qualified_name(expr: &crate::Expr<ac_runtime_values::NonFunctionValue>) -> Option<String>
```



