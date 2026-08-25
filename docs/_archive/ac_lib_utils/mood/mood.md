**handl_lib_utils > mood > mood**

# Module: mood::mood

## Contents

**Structs**

- [`MoodCnd`](#moodcnd)
- [`MoodInd`](#moodind)
- [`MoodInt`](#moodint)
- [`MoodModule`](#moodmodule)
- [`MoodNeg`](#moodneg)
- [`MoodNorm`](#moodnorm)
- [`MoodPot`](#moodpot)

**Functions**

- [`as_kind`](#as_kind)
- [`bool_value`](#bool_value)
- [`complement`](#complement)
- [`err_result`](#err_result)
- [`matches`](#matches)
- [`null_value`](#null_value)
- [`ok_result`](#ok_result)
- [`project`](#project)
- [`require_arg_count`](#require_arg_count)
- [`type_name`](#type_name)

---

## handl_lib_utils::mood::mood::MoodCnd

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> MoodCnd`



## handl_lib_utils::mood::mood::MoodInd

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> MoodInd`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::mood::mood::MoodInt

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> MoodInt`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::mood::mood::MoodModule

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> MoodModule`
- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`



## handl_lib_utils::mood::mood::MoodNeg

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> MoodNeg`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::mood::mood::MoodNorm

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> MoodNorm`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::mood::mood::MoodPot

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> MoodPot`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::mood::mood::as_kind

*Function*

```rust
fn as_kind(value: &ac_runtime_values::NonFunctionValue) -> Result<&str, ac_runtime_values::Error>
```



## handl_lib_utils::mood::mood::bool_value

*Function*

```rust
fn bool_value(b: bool) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_utils::mood::mood::complement

*Function*

```rust
fn complement(value: &ac_runtime_values::NonFunctionValue, kind: &str) -> Option<ac_runtime_values::NonFunctionValue>
```



## handl_lib_utils::mood::mood::err_result

*Function*

```rust
fn err_result(message: &str) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_utils::mood::mood::matches

*Function*

```rust
fn matches(value: &ac_runtime_values::NonFunctionValue, kind: &str) -> bool
```



## handl_lib_utils::mood::mood::null_value

*Function*

```rust
fn null_value() -> ac_runtime_values::NonFunctionValue
```



## handl_lib_utils::mood::mood::ok_result

*Function*

```rust
fn ok_result(value: ac_runtime_values::NonFunctionValue) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_utils::mood::mood::project

*Function*

```rust
fn project(value: &ac_runtime_values::NonFunctionValue, kind: &str) -> Option<ac_runtime_values::NonFunctionValue>
```



## handl_lib_utils::mood::mood::require_arg_count

*Function*

```rust
fn require_arg_count(args: &[ac_runtime_values::NonFunctionValue], expected: usize) -> Result<(), ac_runtime_values::Error>
```



## handl_lib_utils::mood::mood::type_name

*Function*

```rust
fn type_name(value: &ac_runtime_values::NonFunctionValue) -> &'static str
```



