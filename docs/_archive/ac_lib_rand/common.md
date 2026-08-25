**handl_lib_rand > common**

# Module: common

## Contents

**Functions**

- [`array`](#array)
- [`bool_val`](#bool_val)
- [`num`](#num)
- [`require_array`](#require_array) - Require an array argument at `idx`.
- [`require_int`](#require_int) - Require an integer argument at `idx`.
- [`require_number`](#require_number) - Require a number argument at `idx`.
- [`s`](#s)

---

## handl_lib_rand::common::array

*Function*

```rust
fn array(values: Vec<serde_json::Value>) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_rand::common::bool_val

*Function*

```rust
fn bool_val(value: bool) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_rand::common::num

*Function*

```rust
fn num<impl Into<f64>>(value: impl Trait) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_rand::common::require_array

*Function*

Require an array argument at `idx`.

```rust
fn require_array(args: &[ac_runtime_values::NonFunctionValue], idx: usize) -> Result<&[serde_json::Value], ac_runtime_values::Error>
```



## handl_lib_rand::common::require_int

*Function*

Require an integer argument at `idx`.

```rust
fn require_int(args: &[ac_runtime_values::NonFunctionValue], idx: usize) -> Result<i64, ac_runtime_values::Error>
```



## handl_lib_rand::common::require_number

*Function*

Require a number argument at `idx`.

```rust
fn require_number(args: &[ac_runtime_values::NonFunctionValue], idx: usize) -> Result<f64, ac_runtime_values::Error>
```



## handl_lib_rand::common::s

*Function*

```rust
fn s<impl Into<String>>(value: impl Trait) -> ac_runtime_values::NonFunctionValue
```



