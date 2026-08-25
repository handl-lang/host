**ac_runtime_canonical > continuation**

# Module: continuation

## Contents

**Structs**

- [`Continuation`](#continuation) - A serializable continuation of a paused evaluation.
- [`Frame`](#frame) - A single frame in the evaluation stack.

**Enums**

- [`EvalResult`](#evalresult) - The result of a resumable evaluation step.
- [`StepResult`](#stepresult) - Internal result of a single evaluation step.

**Functions**

- [`expr_from_json`](#expr_from_json)
- [`expr_to_json`](#expr_to_json)
- [`frame_from_json`](#frame_from_json)
- [`handler_clause_from_json`](#handler_clause_from_json)
- [`handler_clause_to_json`](#handler_clause_to_json)

**Type Aliases**

- [`RuntimeExpr`](#runtimeexpr)
- [`RuntimeHandlerClause`](#runtimehandlerclause)

---

## ac_runtime_canonical::continuation::Continuation

*Struct*

A serializable continuation of a paused evaluation.

Contains the frame stack at the point where evaluation was
paused. All data is JSON-compatible (function names are strings,
values are `serde_json::Value`, expressions are `Expr<NonFunctionValue>`).

**Fields:**
- `frames: Vec<Frame>` - The frame stack, bottom to top.

**Methods:**

- `fn empty() -> Self` - Creates an empty continuation (no frames).
- `fn is_empty(self: &Self) -> bool` - Returns `true` if there are no frames to resume.
- `fn to_json(self: &Self) -> serde_json::Value` - Serializes the continuation to a JSON value.
- `fn from_json(value: &serde_json::Value) -> Result<Self, Error<NonFunctionValue>>` - Deserializes a continuation from a JSON value.

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &Continuation) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> Continuation`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## ac_runtime_canonical::continuation::EvalResult

*Enum*

The result of a resumable evaluation step.

**Variants:**
- `Done(ac_runtime_values::NonFunctionValue)` - Evaluation completed successfully.
- `Yielded{ value: ac_runtime_values::NonFunctionValue, continuation: Continuation }` - Evaluation paused — a host function yielded.
- `Error(crate::Error<ac_runtime_values::NonFunctionValue>)` - Evaluation failed with an error.

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &EvalResult) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> EvalResult`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## ac_runtime_canonical::continuation::Frame

*Struct*

A single frame in the evaluation stack.

Records the state of a `Call` expression that is mid-evaluation:
some arguments have been evaluated, others are pending.

**Fields:**
- `name: String` - The function name being called.
- `evaluated: Vec<ac_runtime_values::NonFunctionValue>` - Arguments that have already been evaluated.
- `pending: Vec<crate::Expr<ac_runtime_values::NonFunctionValue>>` - Arguments that still need evaluation (remaining, in order).
- `handlers: Vec<crate::HandlerClause<ac_runtime_values::NonFunctionValue>>` - Handler clauses active for this frame (from enclosing Handle exprs).

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &Frame) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> Frame`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## ac_runtime_canonical::continuation::RuntimeExpr

*Type Alias*: `crate::Expr<ac_runtime_values::NonFunctionValue>`



## ac_runtime_canonical::continuation::RuntimeHandlerClause

*Type Alias*: `crate::HandlerClause<ac_runtime_values::NonFunctionValue>`



## ac_runtime_canonical::continuation::StepResult

*Enum*

Internal result of a single evaluation step.

**Variants:**
- `Done(ac_runtime_values::NonFunctionValue)` - Evaluation completed with a value.
- `Yield{ value: ac_runtime_values::NonFunctionValue, frames: Vec<Frame> }` - A host function yielded. The `frames` vector is the frame
- `Error(crate::Error<ac_runtime_values::NonFunctionValue>)` - An error occurred.



## ac_runtime_canonical::continuation::expr_from_json

*Function*

```rust
fn expr_from_json(value: &serde_json::Value) -> Result<crate::Expr<ac_runtime_values::NonFunctionValue>, crate::Error<ac_runtime_values::NonFunctionValue>>
```



## ac_runtime_canonical::continuation::expr_to_json

*Function*

```rust
fn expr_to_json(expr: &crate::Expr<ac_runtime_values::NonFunctionValue>) -> serde_json::Value
```



## ac_runtime_canonical::continuation::frame_from_json

*Function*

```rust
fn frame_from_json(value: &serde_json::Value) -> Result<Frame, crate::Error<ac_runtime_values::NonFunctionValue>>
```



## ac_runtime_canonical::continuation::handler_clause_from_json

*Function*

```rust
fn handler_clause_from_json(value: &serde_json::Value) -> Result<crate::HandlerClause<ac_runtime_values::NonFunctionValue>, crate::Error<ac_runtime_values::NonFunctionValue>>
```



## ac_runtime_canonical::continuation::handler_clause_to_json

*Function*

```rust
fn handler_clause_to_json(clause: &crate::HandlerClause<ac_runtime_values::NonFunctionValue>) -> serde_json::Value
```



