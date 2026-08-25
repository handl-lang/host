**handl_lib_datetime > datetime > bounds**

# Module: datetime::bounds

## Contents

**Macros**

- [`bound_fn`](#bound_fn)

**Structs**

- [`EndOfDay`](#endofday)
- [`EndOfHour`](#endofhour)
- [`EndOfMonth`](#endofmonth)
- [`EndOfYear`](#endofyear)
- [`StartOfDay`](#startofday)
- [`StartOfHour`](#startofhour)
- [`StartOfMonth`](#startofmonth)
- [`StartOfYear`](#startofyear)

---

## handl_lib_datetime::datetime::bounds::EndOfDay

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> EndOfDay`



## handl_lib_datetime::datetime::bounds::EndOfHour

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> EndOfHour`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::bounds::EndOfMonth

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> EndOfMonth`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::bounds::EndOfYear

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> EndOfYear`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::bounds::StartOfDay

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> StartOfDay`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::bounds::StartOfHour

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> StartOfHour`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::bounds::StartOfMonth

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> StartOfMonth`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::bounds::StartOfYear

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> StartOfYear`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::bounds::bound_fn

*Declarative Macro*

```rust
macro_rules! bound_fn {
    ($struct_name:ident, $body:expr) => { ... };
}
```



