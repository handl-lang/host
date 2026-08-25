**handl_lib_utils > obj > common**

# Module: obj::common

## Contents

**Functions**

- [`arr`](#arr)
- [`b`](#b)
- [`bind_param`](#bind_param)
- [`deep_merge_values`](#deep_merge_values)
- [`num`](#num)
- [`obj_value`](#obj_value)
- [`require_array`](#require_array)
- [`require_function_value`](#require_function_value)
- [`require_object`](#require_object)
- [`require_one_arg`](#require_one_arg)
- [`require_string`](#require_string)
- [`require_three_args`](#require_three_args)
- [`require_two_args`](#require_two_args)
- [`set_path_recursive`](#set_path_recursive)
- [`update_path_recursive`](#update_path_recursive)

---

## handl_lib_utils::obj::common::arr

*Function*

```rust
fn arr(items: &[serde_json::Value]) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_utils::obj::common::b

*Function*

```rust
fn b(value: bool) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_utils::obj::common::bind_param

*Function*

```rust
fn bind_param(fn_val: &ac_runtime_values::FunctionValue, value: &serde_json::Value) -> Vec<(String, ac_runtime_values::NonFunctionValue)>
```



## handl_lib_utils::obj::common::deep_merge_values

*Function*

```rust
fn deep_merge_values(a: &serde_json::Value, b: &serde_json::Value) -> serde_json::Value
```



## handl_lib_utils::obj::common::num

*Function*

```rust
fn num(value: f64) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_utils::obj::common::obj_value

*Function*

```rust
fn obj_value(map: serde_json::Map<String, serde_json::Value>) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_utils::obj::common::require_array

*Function*

```rust
fn require_array(arg: &ac_runtime_values::NonFunctionValue) -> Result<&[serde_json::Value], ac_runtime_values::Error>
```



## handl_lib_utils::obj::common::require_function_value

*Function*

```rust
fn require_function_value(arg: &ac_runtime_values::NonFunctionValue) -> Result<ac_runtime_values::FunctionValue, ac_runtime_values::Error>
```



## handl_lib_utils::obj::common::require_object

*Function*

```rust
fn require_object(arg: &ac_runtime_values::NonFunctionValue) -> Result<&serde_json::Map<String, serde_json::Value>, ac_runtime_values::Error>
```



## handl_lib_utils::obj::common::require_one_arg

*Function*

```rust
fn require_one_arg(args: &[ac_runtime_values::NonFunctionValue]) -> Result<&ac_runtime_values::NonFunctionValue, ac_runtime_values::Error>
```



## handl_lib_utils::obj::common::require_string

*Function*

```rust
fn require_string(arg: &ac_runtime_values::NonFunctionValue) -> Result<String, ac_runtime_values::Error>
```



## handl_lib_utils::obj::common::require_three_args

*Function*

```rust
fn require_three_args(args: &[ac_runtime_values::NonFunctionValue]) -> Result<(&ac_runtime_values::NonFunctionValue, &ac_runtime_values::NonFunctionValue, &ac_runtime_values::NonFunctionValue), ac_runtime_values::Error>
```



## handl_lib_utils::obj::common::require_two_args

*Function*

```rust
fn require_two_args(args: &[ac_runtime_values::NonFunctionValue]) -> Result<(&ac_runtime_values::NonFunctionValue, &ac_runtime_values::NonFunctionValue), ac_runtime_values::Error>
```



## handl_lib_utils::obj::common::set_path_recursive

*Function*

```rust
fn set_path_recursive(value: serde_json::Value, segments: &[&str], new_value: serde_json::Value) -> serde_json::Value
```



## handl_lib_utils::obj::common::update_path_recursive

*Function*

```rust
fn update_path_recursive(value: serde_json::Value, segments: &[&str], fn_val: &ac_runtime_values::FunctionValue, ctx: & mut dyn ac_runtime::Context) -> Result<serde_json::Value, ac_runtime_values::Error>
```



