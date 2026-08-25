**handl_lib_utils > type > utilities**

# Module: type::utilities

## Contents

**Structs**

- [`Coalesce`](#coalesce)
- [`Default`](#default)
- [`Identity`](#identity)

**Functions**

- [`require_one_arg`](#require_one_arg)
- [`require_two_args`](#require_two_args)

---

## handl_lib_utils::type::utilities::Coalesce

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Coalesce`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::type::utilities::Default

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Default`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::type::utilities::Identity

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Identity`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::type::utilities::require_one_arg

*Function*

```rust
fn require_one_arg(args: &[ac_runtime_values::NonFunctionValue]) -> Result<&ac_runtime_values::NonFunctionValue, ac_runtime_values::Error>
```



## handl_lib_utils::type::utilities::require_two_args

*Function*

```rust
fn require_two_args(args: &[ac_runtime_values::NonFunctionValue]) -> Result<(&ac_runtime_values::NonFunctionValue, &ac_runtime_values::NonFunctionValue), ac_runtime_values::Error>
```



