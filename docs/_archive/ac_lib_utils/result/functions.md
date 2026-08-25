**handl_lib_utils > result > functions**

# Module: result::functions

## Contents

**Structs**

- [`Err`](#err)
- [`IsErr`](#iserr)
- [`IsOk`](#isok)
- [`Ok`](#ok)

**Functions**

- [`as_result_tag`](#as_result_tag)

---

## handl_lib_utils::result::functions::Err

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Err`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::result::functions::IsErr

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> IsErr`



## handl_lib_utils::result::functions::IsOk

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> IsOk`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::result::functions::Ok

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Ok`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::result::functions::as_result_tag

*Function*

```rust
fn as_result_tag(value: &serde_json::Value) -> Option<&str>
```



