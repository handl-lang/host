**handl_lib_utils > str > search**

# Module: str::search

## Contents

**Structs**

- [`Contains`](#contains)
- [`Count`](#count)
- [`EndsWith`](#endswith)
- [`Find`](#find)
- [`Levenshtein`](#levenshtein)
- [`RFind`](#rfind)
- [`Similarity`](#similarity)
- [`StartsWith`](#startswith)

**Functions**

- [`levenshtein`](#levenshtein)

---

## handl_lib_utils::str::search::Contains

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Contains`
- **Function**
  - `fn call(self: &Self, args: &[ac_runtime_values::NonFunctionValue], _ctx: & mut dyn ac_runtime::Context) -> Result<ac_runtime_values::NonFunctionValue, ac_runtime_values::Error>`



## handl_lib_utils::str::search::Count

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Count`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::str::search::EndsWith

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> EndsWith`
- **Function**
  - `fn call(self: &Self, args: &[ac_runtime_values::NonFunctionValue], _ctx: & mut dyn ac_runtime::Context) -> Result<ac_runtime_values::NonFunctionValue, ac_runtime_values::Error>`



## handl_lib_utils::str::search::Find

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Find`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::str::search::Levenshtein

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Levenshtein`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::str::search::RFind

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> RFind`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::str::search::Similarity

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Similarity`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::str::search::StartsWith

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> StartsWith`
- **Function**
  - `fn call(self: &Self, args: &[ac_runtime_values::NonFunctionValue], _ctx: & mut dyn ac_runtime::Context) -> Result<ac_runtime_values::NonFunctionValue, ac_runtime_values::Error>`



## handl_lib_utils::str::search::levenshtein

*Function*

```rust
fn levenshtein(a: &[char], b: &[char]) -> usize
```



