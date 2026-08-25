**handl_lib_datetime > datetime > arithmetic**

# Module: datetime::arithmetic

## Contents

**Structs**

- [`AddDays`](#adddays)
- [`AddHours`](#addhours)
- [`AddMinutes`](#addminutes)
- [`AddMonths`](#addmonths)
- [`AddSeconds`](#addseconds)
- [`AddWeeks`](#addweeks)
- [`AddYears`](#addyears)

**Functions**

- [`add_months_to_date`](#add_months_to_date)

---

## handl_lib_datetime::datetime::arithmetic::AddDays

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> AddDays`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::arithmetic::AddHours

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> AddHours`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::arithmetic::AddMinutes

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> AddMinutes`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::arithmetic::AddMonths

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> AddMonths`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::arithmetic::AddSeconds

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> AddSeconds`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::arithmetic::AddWeeks

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> AddWeeks`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_datetime::datetime::arithmetic::AddYears

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> AddYears`



## handl_lib_datetime::datetime::arithmetic::add_months_to_date

*Function*

```rust
fn add_months_to_date(dt: chrono::DateTime<chrono::Utc>, months: i64) -> Result<chrono::DateTime<chrono::Utc>, ac_runtime_values::Error>
```



