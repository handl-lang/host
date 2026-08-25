**handl_lib_utils > bool > bool**

# Module: bool::bool

## Contents

**Macros**

- [`bool_binary_fn`](#bool_binary_fn)
- [`bool_unary_fn`](#bool_unary_fn)

**Structs**

- [`And`](#and)
- [`AndLazy`](#andlazy) - Lazy `and(a, b)` — stops at the first `false`.
- [`BoolModule`](#boolmodule) - The `bool` module — boolean utility functions.
- [`Eq`](#eq)
- [`Implies`](#implies)
- [`Nand`](#nand)
- [`Nor`](#nor)
- [`Not`](#not)
- [`Or`](#or)
- [`OrLazy`](#orlazy) - Lazy `or(a, b)` — stops at the first `true`.
- [`Xor`](#xor)

**Functions**

- [`require_bool`](#require_bool)
- [`require_one_bool`](#require_one_bool)
- [`require_two_bools`](#require_two_bools)
- [`to_bool`](#to_bool)

---

## handl_lib_utils::bool::bool::And

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> And`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::bool::bool::AndLazy

*Struct*

Lazy `and(a, b)` — stops at the first `false`.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> AndLazy`
- **LazyFunction**
  - `fn call(self: &Self, args: &[Expr<NonFunctionValue>], ctx: & mut dyn Context, evaluate: & mut dyn FnMut) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::bool::bool::BoolModule

*Struct*

The `bool` module — boolean utility functions.

Provides functions callable as `bool.and(a, b)`, `bool.or(a, b)`,
etc. in handl expressions.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> BoolModule`
- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
  - `fn lazy_function(self: &Self, name: &str) -> Option<&dyn LazyFunction>`



## handl_lib_utils::bool::bool::Eq

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Eq`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::bool::bool::Implies

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Implies`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::bool::bool::Nand

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Nand`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::bool::bool::Nor

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Nor`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::bool::bool::Not

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Not`



## handl_lib_utils::bool::bool::Or

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Or`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::bool::bool::OrLazy

*Struct*

Lazy `or(a, b)` — stops at the first `true`.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **LazyFunction**
  - `fn call(self: &Self, args: &[Expr<NonFunctionValue>], ctx: & mut dyn Context, evaluate: & mut dyn FnMut) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> OrLazy`



## handl_lib_utils::bool::bool::Xor

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Xor`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::bool::bool::bool_binary_fn

*Declarative Macro*

```rust
macro_rules! bool_binary_fn {
    ($struct_name:ident, $body:expr) => { ... };
}
```



## handl_lib_utils::bool::bool::bool_unary_fn

*Declarative Macro*

```rust
macro_rules! bool_unary_fn {
    ($struct_name:ident, $body:expr) => { ... };
}
```



## handl_lib_utils::bool::bool::require_bool

*Function*

```rust
fn require_bool(arg: &ac_runtime_values::NonFunctionValue) -> Result<bool, ac_runtime_values::Error>
```



## handl_lib_utils::bool::bool::require_one_bool

*Function*

```rust
fn require_one_bool(args: &[ac_runtime_values::NonFunctionValue]) -> Result<bool, ac_runtime_values::Error>
```



## handl_lib_utils::bool::bool::require_two_bools

*Function*

```rust
fn require_two_bools(args: &[ac_runtime_values::NonFunctionValue]) -> Result<(bool, bool), ac_runtime_values::Error>
```



## handl_lib_utils::bool::bool::to_bool

*Function*

```rust
fn to_bool(value: bool) -> ac_runtime_values::NonFunctionValue
```



