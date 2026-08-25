**handl_lib_datetime > datetime > components**

# Module: datetime::components

## Contents

**Macros**

- [`component_fn`](#component_fn)

**Structs**

- [`Day`](#day)
- [`DayOfYear`](#dayofyear)
- [`Hour`](#hour)
- [`IsLeapYear`](#isleapyear)
- [`IsWeekend`](#isweekend)
- [`Minute`](#minute)
- [`Month`](#month)
- [`Quarter`](#quarter)
- [`Second`](#second)
- [`WeekOfYear`](#weekofyear)
- [`Weekday`](#weekday)
- [`Year`](#year)

---

## handl_lib_datetime::datetime::components::Day

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Day`



## handl_lib_datetime::datetime::components::DayOfYear

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> DayOfYear`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::components::Hour

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Hour`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::components::IsLeapYear

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> IsLeapYear`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::components::IsWeekend

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> IsWeekend`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::components::Minute

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Minute`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::components::Month

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Month`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::components::Quarter

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Quarter`



## handl_lib_datetime::datetime::components::Second

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Second`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::components::WeekOfYear

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> WeekOfYear`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::components::Weekday

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Weekday`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::components::Year

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Year`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::components::component_fn

*Declarative Macro*

```rust
macro_rules! component_fn {
    ($struct_name:ident, $extract:expr) => { ... };
}
```



