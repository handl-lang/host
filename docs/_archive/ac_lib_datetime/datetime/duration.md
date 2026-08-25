**handl_lib_datetime > datetime > duration**

# Module: datetime::duration

## Contents

**Structs**

- [`DurationBetween`](#durationbetween)
- [`DurationDays`](#durationdays)
- [`DurationHours`](#durationhours)
- [`DurationMinutes`](#durationminutes)
- [`DurationSeconds`](#durationseconds)

**Functions**

- [`duration_secs_f64`](#duration_secs_f64)

---

## handl_lib_datetime::datetime::duration::DurationBetween

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> DurationBetween`



## handl_lib_datetime::datetime::duration::DurationDays

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> DurationDays`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::duration::DurationHours

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> DurationHours`



## handl_lib_datetime::datetime::duration::DurationMinutes

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> DurationMinutes`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::duration::DurationSeconds

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> DurationSeconds`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::duration::duration_secs_f64

*Function*

```rust
fn duration_secs_f64(dt_a: chrono::DateTime<chrono::Utc>, dt_b: chrono::DateTime<chrono::Utc>) -> f64
```



