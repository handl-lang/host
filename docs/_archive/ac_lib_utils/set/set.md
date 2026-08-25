**handl_lib_utils > set > set**

# Module: set::set

## Contents

**Structs**

- [`Difference`](#difference)
- [`Intersection`](#intersection)
- [`SetModule`](#setmodule) - The `set` module — set-theoretic operations on arrays.
- [`SymmetricDifference`](#symmetricdifference)
- [`Union`](#union)
- [`Unique`](#unique)
- [`UniqueBy`](#uniqueby)
- [`Without`](#without)

**Functions**

- [`arr`](#arr)
- [`bind_param`](#bind_param)
- [`dedup_ordered`](#dedup_ordered) - Deduplicate a slice while preserving first-occurrence order.
- [`require_array`](#require_array)
- [`require_function_value`](#require_function_value)
- [`require_one_array`](#require_one_array)
- [`require_two_arrays`](#require_two_arrays)
- [`value_key`](#value_key) - Serialize a `serde_json::Value` to a string for use as a `HashSet` key.

---

## handl_lib_utils::set::set::Difference

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Difference`



## handl_lib_utils::set::set::Intersection

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Intersection`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::set::set::SetModule

*Struct*

The `set` module — set-theoretic operations on arrays.

All functions accept JSON arrays and return JSON arrays with
duplicates removed (set-correct semantics). Internally uses
`HashSet` for O(n) lookup.

Callable as `set.union(a, b)`, `set.intersection(a, b)`, etc.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
- **Clone**
  - `fn clone(self: &Self) -> SetModule`



## handl_lib_utils::set::set::SymmetricDifference

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> SymmetricDifference`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::set::set::Union

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Union`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::set::set::Unique

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Unique`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::set::set::UniqueBy

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> UniqueBy`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::set::set::Without

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Without`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::set::set::arr

*Function*

```rust
fn arr(items: &[serde_json::Value]) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_utils::set::set::bind_param

*Function*

```rust
fn bind_param(fn_val: &ac_runtime_values::FunctionValue, value: &serde_json::Value) -> Vec<(String, ac_runtime_values::NonFunctionValue)>
```



## handl_lib_utils::set::set::dedup_ordered

*Function*

Deduplicate a slice while preserving first-occurrence order.

```rust
fn dedup_ordered(items: &[serde_json::Value]) -> Vec<serde_json::Value>
```



## handl_lib_utils::set::set::require_array

*Function*

```rust
fn require_array(arg: &ac_runtime_values::NonFunctionValue) -> Result<&[serde_json::Value], ac_runtime_values::Error>
```



## handl_lib_utils::set::set::require_function_value

*Function*

```rust
fn require_function_value(arg: &ac_runtime_values::NonFunctionValue) -> Result<ac_runtime_values::FunctionValue, ac_runtime_values::Error>
```



## handl_lib_utils::set::set::require_one_array

*Function*

```rust
fn require_one_array(args: &[ac_runtime_values::NonFunctionValue]) -> Result<&[serde_json::Value], ac_runtime_values::Error>
```



## handl_lib_utils::set::set::require_two_arrays

*Function*

```rust
fn require_two_arrays(args: &[ac_runtime_values::NonFunctionValue]) -> Result<(&[serde_json::Value], &[serde_json::Value]), ac_runtime_values::Error>
```



## handl_lib_utils::set::set::value_key

*Function*

Serialize a `serde_json::Value` to a string for use as a `HashSet` key.
This handles all JSON types (numbers, strings, bools, null, arrays, objects).

```rust
fn value_key(value: &serde_json::Value) -> String
```



