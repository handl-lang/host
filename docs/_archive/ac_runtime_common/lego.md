**ac_runtime_common > lego**

# Module: lego

## Contents

**Structs**

- [`LegoVm`](#legovm) - A virtual machine assembled from an ordered list of expression handlers.

**Traits**

- [`ExprHandler`](#exprhandler) - A single, pluggable evaluator for one expression shape or language capability.

**Type Aliases**

- [`RuntimeExpr`](#runtimeexpr)

---

## ac_runtime_common::lego::ExprHandler

*Trait*

A single, pluggable evaluator for one expression shape or language capability.

**Methods:**

- `can_handle`: Returns `true` if this handler should evaluate `expr`.
- `eval`: Evaluates `expr` using the host and the given dispatch closure for sub-expressions.



## ac_runtime_common::lego::LegoVm

*Struct*

A virtual machine assembled from an ordered list of expression handlers.

**Generic Parameters:**
- H

**Fields:**
- `host: H`
- `handlers: Vec<Box<dyn ExprHandler>>`

**Methods:**

- `fn new(host: H) -> Self` - Creates a new `LegoVm` with the given host and no handlers.
- `fn with<impl ExprHandler>(self: Self, handler: impl Trait) -> Self` - Adds a handler to the end of the dispatch list.
- `fn host(self: &Self) -> &H` - Returns a reference to the host.
- `fn eval(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>, ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>` - Evaluates an expression by dispatching to the first matching handler.

**Trait Implementations:**

- **VirtualMachine**
  - `fn eval(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>, ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>`
  - `fn eval_async(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>, ctx: & mut dyn Context) -> impl Trait`



## ac_runtime_common::lego::RuntimeExpr

*Type Alias*: `ac_runtime::Expr<ac_runtime_values::NonFunctionValue>`



