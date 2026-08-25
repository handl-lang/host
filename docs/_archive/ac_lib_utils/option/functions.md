**handl_lib_utils > option > functions**

# Module: option::functions

## Contents

**Structs**

- [`And`](#and)
- [`Filter`](#filter)
- [`FlatMap`](#flatmap)
- [`IsNone`](#isnone)
- [`IsSome`](#issome)
- [`Map`](#map)
- [`MapOr`](#mapor)
- [`NoneFn`](#nonefn)
- [`Or`](#or)
- [`OrElse`](#orelse)
- [`SomeFn`](#somefn)
- [`Unwrap`](#unwrap)
- [`UnwrapOr`](#unwrapor)
- [`UnwrapOrElse`](#unwraporelse)
- [`Xor`](#xor)

**Functions**

- [`as_option_tag`](#as_option_tag)
- [`get_value`](#get_value)
- [`is_none`](#is_none)
- [`is_some`](#is_some)
- [`require_option`](#require_option)

---

## handl_lib_utils::option::functions::And

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> And`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::option::functions::Filter

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Filter`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::option::functions::FlatMap

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> FlatMap`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::option::functions::IsNone

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> IsNone`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::option::functions::IsSome

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> IsSome`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::option::functions::Map

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Map`



## handl_lib_utils::option::functions::MapOr

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> MapOr`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::option::functions::NoneFn

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> NoneFn`
- **Function**
  - `fn call(self: &Self, _args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::option::functions::Or

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Or`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::option::functions::OrElse

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> OrElse`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::option::functions::SomeFn

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> SomeFn`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::option::functions::Unwrap

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Unwrap`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::option::functions::UnwrapOr

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> UnwrapOr`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::option::functions::UnwrapOrElse

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> UnwrapOrElse`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::option::functions::Xor

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Xor`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::option::functions::as_option_tag

*Function*

```rust
fn as_option_tag(value: &serde_json::Value) -> Option<&str>
```



## handl_lib_utils::option::functions::get_value

*Function*

```rust
fn get_value(arg: &ac_runtime_values::NonFunctionValue) -> Option<&serde_json::Value>
```



## handl_lib_utils::option::functions::is_none

*Function*

```rust
fn is_none(value: &serde_json::Value) -> bool
```



## handl_lib_utils::option::functions::is_some

*Function*

```rust
fn is_some(value: &serde_json::Value) -> bool
```



## handl_lib_utils::option::functions::require_option

*Function*

```rust
fn require_option(arg: &ac_runtime_values::NonFunctionValue) -> Result<(), ac_runtime_values::Error>
```



