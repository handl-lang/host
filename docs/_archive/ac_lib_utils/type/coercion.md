**handl_lib_utils > type > coercion**

# Module: type::coercion

## Contents

**Structs**

- [`ToBool`](#tobool)
- [`ToNumber`](#tonumber)
- [`ToString`](#tostring)

**Functions**

- [`number_to_string`](#number_to_string)
- [`require_one_arg`](#require_one_arg)

---

## handl_lib_utils::type::coercion::ToBool

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> ToBool`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::type::coercion::ToNumber

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> ToNumber`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::type::coercion::ToString

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> ToString`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::type::coercion::number_to_string

*Function*

```rust
fn number_to_string(n: f64) -> String
```



## handl_lib_utils::type::coercion::require_one_arg

*Function*

```rust
fn require_one_arg(args: &[ac_runtime_values::NonFunctionValue]) -> Result<&ac_runtime_values::NonFunctionValue, ac_runtime_values::Error>
```



