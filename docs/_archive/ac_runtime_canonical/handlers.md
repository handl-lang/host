**ac_runtime_canonical > handlers**

# Module: handlers

## Contents

**Structs**

- [`ApplyHandler`](#applyhandler) - Handles calls to an arbitrary function value.
- [`ArrayHandler`](#arrayhandler) - Handles array literals.
- [`AssignHandler`](#assignhandler) - Handles assignment expressions.
- [`CallHandler`](#callhandler) - Handles function and operation calls.
- [`FieldAccessHandler`](#fieldaccesshandler) - Handles postfix field access expressions.
- [`FunctionHandler`](#functionhandler) - Handles first-class function literals.
- [`HandleHandler`](#handlehandler) - Handles effect-handler expressions.
- [`IndexHandler`](#indexhandler) - Handles postfix index expressions.
- [`ObjectHandler`](#objecthandler) - Handles object literals.
- [`ValueHandler`](#valuehandler) - Handles literal value expressions.
- [`VarHandler`](#varhandler) - Handles bare identifier values.

**Functions**

- [`canonical_vm`](#canonical_vm) - Builds a canonical `LegoVm` with the default expression handlers.

**Type Aliases**

- [`RuntimeExpr`](#runtimeexpr)

---

## ac_runtime_canonical::handlers::ApplyHandler

*Struct*

Handles calls to an arbitrary function value.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **ExprHandler**
  - `fn can_handle(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>) -> bool`
  - `fn eval(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>, ctx: & mut dyn Context, host: &dyn Host, dispatch: & mut dyn FnMut) -> Result<NonFunctionValue, Error<NonFunctionValue>>`
- **Clone**
  - `fn clone(self: &Self) -> ApplyHandler`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> ApplyHandler`



## ac_runtime_canonical::handlers::ArrayHandler

*Struct*

Handles array literals.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **ExprHandler**
  - `fn can_handle(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>) -> bool`
  - `fn eval(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>, ctx: & mut dyn Context, _host: &dyn Host, dispatch: & mut dyn FnMut) -> Result<NonFunctionValue, Error<NonFunctionValue>>`
- **Clone**
  - `fn clone(self: &Self) -> ArrayHandler`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> ArrayHandler`



## ac_runtime_canonical::handlers::AssignHandler

*Struct*

Handles assignment expressions.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **ExprHandler**
  - `fn can_handle(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>) -> bool`
  - `fn eval(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>, ctx: & mut dyn Context, host: &dyn Host, dispatch: & mut dyn FnMut) -> Result<NonFunctionValue, Error<NonFunctionValue>>`
- **Clone**
  - `fn clone(self: &Self) -> AssignHandler`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> AssignHandler`



## ac_runtime_canonical::handlers::CallHandler

*Struct*

Handles function and operation calls.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **ExprHandler**
  - `fn can_handle(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>) -> bool`
  - `fn eval(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>, ctx: & mut dyn Context, host: &dyn Host, dispatch: & mut dyn FnMut) -> Result<NonFunctionValue, Error<NonFunctionValue>>`
- **Clone**
  - `fn clone(self: &Self) -> CallHandler`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> CallHandler`



## ac_runtime_canonical::handlers::FieldAccessHandler

*Struct*

Handles postfix field access expressions.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **ExprHandler**
  - `fn can_handle(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>) -> bool`
  - `fn eval(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>, ctx: & mut dyn Context, _host: &dyn Host, dispatch: & mut dyn FnMut) -> Result<NonFunctionValue, Error<NonFunctionValue>>`
- **Clone**
  - `fn clone(self: &Self) -> FieldAccessHandler`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> FieldAccessHandler`



## ac_runtime_canonical::handlers::FunctionHandler

*Struct*

Handles first-class function literals.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **ExprHandler**
  - `fn can_handle(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>) -> bool`
  - `fn eval(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>, ctx: & mut dyn Context, _host: &dyn Host, dispatch: & mut dyn FnMut) -> Result<NonFunctionValue, Error<NonFunctionValue>>`
- **Clone**
  - `fn clone(self: &Self) -> FunctionHandler`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> FunctionHandler`



## ac_runtime_canonical::handlers::HandleHandler

*Struct*

Handles effect-handler expressions.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **ExprHandler**
  - `fn can_handle(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>) -> bool`
  - `fn eval(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>, ctx: & mut dyn Context, host: &dyn Host, _dispatch: & mut dyn FnMut) -> Result<NonFunctionValue, Error<NonFunctionValue>>`
- **Clone**
  - `fn clone(self: &Self) -> HandleHandler`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> HandleHandler`



## ac_runtime_canonical::handlers::IndexHandler

*Struct*

Handles postfix index expressions.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **ExprHandler**
  - `fn can_handle(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>) -> bool`
  - `fn eval(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>, ctx: & mut dyn Context, host: &dyn Host, dispatch: & mut dyn FnMut) -> Result<NonFunctionValue, Error<NonFunctionValue>>`
- **Clone**
  - `fn clone(self: &Self) -> IndexHandler`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> IndexHandler`



## ac_runtime_canonical::handlers::ObjectHandler

*Struct*

Handles object literals.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **ExprHandler**
  - `fn can_handle(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>) -> bool`
  - `fn eval(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>, ctx: & mut dyn Context, _host: &dyn Host, dispatch: & mut dyn FnMut) -> Result<NonFunctionValue, Error<NonFunctionValue>>`
- **Clone**
  - `fn clone(self: &Self) -> ObjectHandler`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> ObjectHandler`



## ac_runtime_canonical::handlers::RuntimeExpr

*Type Alias*: `ac_runtime::Expr<ac_runtime_values::NonFunctionValue>`



## ac_runtime_canonical::handlers::ValueHandler

*Struct*

Handles literal value expressions.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Default**
  - `fn default() -> ValueHandler`
- **Clone**
  - `fn clone(self: &Self) -> ValueHandler`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **ExprHandler**
  - `fn can_handle(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>) -> bool`
  - `fn eval(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>, _ctx: & mut dyn Context, _host: &dyn Host, _dispatch: & mut dyn FnMut) -> Result<NonFunctionValue, Error<NonFunctionValue>>`



## ac_runtime_canonical::handlers::VarHandler

*Struct*

Handles bare identifier values.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **ExprHandler**
  - `fn can_handle(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>) -> bool`
  - `fn eval(self: &Self, expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>, ctx: & mut dyn Context, host: &dyn Host, _dispatch: & mut dyn FnMut) -> Result<NonFunctionValue, Error<NonFunctionValue>>`
- **Clone**
  - `fn clone(self: &Self) -> VarHandler`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> VarHandler`



## ac_runtime_canonical::handlers::canonical_vm

*Function*

Builds a canonical `LegoVm` with the default expression handlers.

```rust
fn canonical_vm<H>(host: H) -> ac_runtime_common::LegoVm<H>
```



