**handl_lib_utils > cmp > cmp**

# Module: cmp::cmp

## Contents

**Structs**

- [`Between`](#between)
- [`Clamp`](#clamp)
- [`CmpModule`](#cmpmodule) - The `cmp` module — comparison and ordering utilities.
- [`Compare`](#compare)
- [`Eq`](#eq)
- [`Ge`](#ge)
- [`Gt`](#gt)
- [`InRange`](#inrange)
- [`IsSorted`](#issorted)
- [`IsSortedDesc`](#issorteddesc)
- [`Le`](#le)
- [`Lt`](#lt)
- [`Max`](#max)
- [`Min`](#min)
- [`Ne`](#ne)
- [`Sort`](#sort)
- [`SortDesc`](#sortdesc)

**Functions**

- [`between_impl`](#between_impl)
- [`compare_nfv`](#compare_nfv)
- [`compare_values`](#compare_values)
- [`is_sorted_impl`](#is_sorted_impl)
- [`require_three`](#require_three)
- [`require_two`](#require_two)
- [`sort_impl`](#sort_impl)
- [`value_kind`](#value_kind)

---

## handl_lib_utils::cmp::cmp::Between

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Between`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::cmp::cmp::Clamp

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Clamp`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::cmp::cmp::CmpModule

*Struct*

The `cmp` module — comparison and ordering utilities.

Provides functions callable as `cmp.eq(a, b)`, `cmp.lt(a, b)`,
`cmp.min(a, b, ...)`, etc. in handl expressions.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
- **Clone**
  - `fn clone(self: &Self) -> CmpModule`



## handl_lib_utils::cmp::cmp::Compare

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Compare`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::cmp::cmp::Eq

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Eq`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::cmp::cmp::Ge

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Ge`



## handl_lib_utils::cmp::cmp::Gt

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Gt`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::cmp::cmp::InRange

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> InRange`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::cmp::cmp::IsSorted

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> IsSorted`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::cmp::cmp::IsSortedDesc

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> IsSortedDesc`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::cmp::cmp::Le

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Le`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::cmp::cmp::Lt

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Lt`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::cmp::cmp::Max

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Max`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::cmp::cmp::Min

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Min`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::cmp::cmp::Ne

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Ne`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::cmp::cmp::Sort

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Sort`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::cmp::cmp::SortDesc

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> SortDesc`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::cmp::cmp::between_impl

*Function*

```rust
fn between_impl(args: &[ac_runtime_values::NonFunctionValue]) -> Result<ac_runtime_values::NonFunctionValue, ac_runtime_values::Error>
```



## handl_lib_utils::cmp::cmp::compare_nfv

*Function*

```rust
fn compare_nfv(a: &ac_runtime_values::NonFunctionValue, b: &ac_runtime_values::NonFunctionValue) -> std::cmp::Ordering
```



## handl_lib_utils::cmp::cmp::compare_values

*Function*

```rust
fn compare_values(a: &serde_json::Value, b: &serde_json::Value) -> std::cmp::Ordering
```



## handl_lib_utils::cmp::cmp::is_sorted_impl

*Function*

```rust
fn is_sorted_impl(args: &[ac_runtime_values::NonFunctionValue], descending: bool) -> Result<ac_runtime_values::NonFunctionValue, ac_runtime_values::Error>
```



## handl_lib_utils::cmp::cmp::require_three

*Function*

```rust
fn require_three(args: &[ac_runtime_values::NonFunctionValue]) -> Result<(&ac_runtime_values::NonFunctionValue, &ac_runtime_values::NonFunctionValue, &ac_runtime_values::NonFunctionValue), ac_runtime_values::Error>
```



## handl_lib_utils::cmp::cmp::require_two

*Function*

```rust
fn require_two(args: &[ac_runtime_values::NonFunctionValue]) -> Result<(&ac_runtime_values::NonFunctionValue, &ac_runtime_values::NonFunctionValue), ac_runtime_values::Error>
```



## handl_lib_utils::cmp::cmp::sort_impl

*Function*

```rust
fn sort_impl(args: &[ac_runtime_values::NonFunctionValue], descending: bool) -> Result<ac_runtime_values::NonFunctionValue, ac_runtime_values::Error>
```



## handl_lib_utils::cmp::cmp::value_kind

*Function*

```rust
fn value_kind(v: &serde_json::Value) -> u8
```



