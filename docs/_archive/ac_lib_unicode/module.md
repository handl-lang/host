**handl_lib_unicode > module**

# Module: module

## Contents

**Structs**

- [`CodePointAt`](#codepointat)
- [`CodePoints`](#codepoints)
- [`Decode`](#decode)
- [`IsValidIdentifier`](#isvalididentifier)
- [`IsXidContinue`](#isxidcontinue)
- [`IsXidStart`](#isxidstart)
- [`UnicodeModule`](#unicodemodule) - The `unicode` module — Unicode primitives for handl expressions.

**Functions**

- [`bool_`](#bool_)
- [`null`](#null)
- [`num`](#num)
- [`require_one_string`](#require_one_string)
- [`require_string_and_index`](#require_string_and_index)

---

## handl_lib_unicode::module::CodePointAt

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> CodePointAt`



## handl_lib_unicode::module::CodePoints

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> CodePoints`



## handl_lib_unicode::module::Decode

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Decode`



## handl_lib_unicode::module::IsValidIdentifier

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> IsValidIdentifier`



## handl_lib_unicode::module::IsXidContinue

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> IsXidContinue`



## handl_lib_unicode::module::IsXidStart

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> IsXidStart`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_unicode::module::UnicodeModule

*Struct*

The `unicode` module — Unicode primitives for handl expressions.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
- **Clone**
  - `fn clone(self: &Self) -> UnicodeModule`



## handl_lib_unicode::module::bool_

*Function*

```rust
fn bool_(value: bool) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_unicode::module::null

*Function*

```rust
fn null() -> ac_runtime_values::NonFunctionValue
```



## handl_lib_unicode::module::num

*Function*

```rust
fn num(value: f64) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_unicode::module::require_one_string

*Function*

```rust
fn require_one_string(args: &[ac_runtime_values::NonFunctionValue]) -> Result<String, ac_runtime_values::Error>
```



## handl_lib_unicode::module::require_string_and_index

*Function*

```rust
fn require_string_and_index(args: &[ac_runtime_values::NonFunctionValue]) -> Result<(String, usize), ac_runtime_values::Error>
```



