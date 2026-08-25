**ac_runtime_example_1 > functions**

# Module: functions

## Contents

**Structs**

- [`Apply`](#apply)
- [`Resolve`](#resolve)
- [`Var`](#var)

**Functions**

- [`require_one_string`](#require_one_string)
- [`require_path`](#require_path)
- [`resolve_path`](#resolve_path)

---

## ac_runtime_example_1::functions::Apply

*Struct*

**Unit Struct**

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Apply`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## ac_runtime_example_1::functions::Resolve

*Struct*

**Unit Struct**

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Resolve`



## ac_runtime_example_1::functions::Var

*Struct*

**Unit Struct**

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Var`



## ac_runtime_example_1::functions::require_one_string

*Function*

```rust
fn require_one_string(args: &[ac_runtime_values::NonFunctionValue]) -> Result<String, ac_runtime_values::Error>
```



## ac_runtime_example_1::functions::require_path

*Function*

```rust
fn require_path(args: &[ac_runtime_values::NonFunctionValue]) -> Result<Vec<&str>, ac_runtime_values::Error>
```



## ac_runtime_example_1::functions::resolve_path

*Function*

```rust
fn resolve_path(value: &ac_runtime_values::NonFunctionValue, path: &[&str]) -> Result<ac_runtime_values::NonFunctionValue, ac_runtime_values::Error>
```



