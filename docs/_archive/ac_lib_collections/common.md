**handl_lib_collections > common**

# Module: common

## Contents

**Functions**

- [`arr`](#arr)
- [`b`](#b)
- [`num`](#num)
- [`obj`](#obj)
- [`require_array`](#require_array) - Require an argument at `idx` to be an array.
- [`require_object`](#require_object) - Require an argument at `idx` to be an object.
- [`require_one_array`](#require_one_array) - Require a single array argument.
- [`require_one_object`](#require_one_object) - Require a single object argument.
- [`require_two_arrays`](#require_two_arrays) - Require two array arguments.

---

## handl_lib_collections::common::arr

*Function*

```rust
fn arr(values: &[serde_json::Value]) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_collections::common::b

*Function*

```rust
fn b(value: bool) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_collections::common::num

*Function*

```rust
fn num<impl Into<f64>>(value: impl Trait) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_collections::common::obj

*Function*

```rust
fn obj(map: serde_json::Map<String, serde_json::Value>) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_collections::common::require_array

*Function*

Require an argument at `idx` to be an array.

```rust
fn require_array(args: &[ac_runtime_values::NonFunctionValue], idx: usize) -> Result<&[serde_json::Value], ac_runtime_values::Error>
```



## handl_lib_collections::common::require_object

*Function*

Require an argument at `idx` to be an object.

```rust
fn require_object(args: &[ac_runtime_values::NonFunctionValue], idx: usize) -> Result<&serde_json::Map<String, serde_json::Value>, ac_runtime_values::Error>
```



## handl_lib_collections::common::require_one_array

*Function*

Require a single array argument.

```rust
fn require_one_array(args: &[ac_runtime_values::NonFunctionValue]) -> Result<&[serde_json::Value], ac_runtime_values::Error>
```



## handl_lib_collections::common::require_one_object

*Function*

Require a single object argument.

```rust
fn require_one_object(args: &[ac_runtime_values::NonFunctionValue]) -> Result<&serde_json::Map<String, serde_json::Value>, ac_runtime_values::Error>
```



## handl_lib_collections::common::require_two_arrays

*Function*

Require two array arguments.

```rust
fn require_two_arrays(args: &[ac_runtime_values::NonFunctionValue]) -> Result<(&[serde_json::Value], &[serde_json::Value]), ac_runtime_values::Error>
```



