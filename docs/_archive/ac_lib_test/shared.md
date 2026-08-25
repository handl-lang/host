**handl_lib_test > shared**

# Module: shared

## Contents

**Functions**

- [`b`](#b)
- [`call_function`](#call_function)
- [`obj`](#obj)
- [`require_array`](#require_array)
- [`require_bool`](#require_bool)
- [`require_function`](#require_function)
- [`require_integer`](#require_integer)
- [`require_len`](#require_len)
- [`require_number`](#require_number)
- [`require_string`](#require_string)
- [`str`](#str)
- [`type_name`](#type_name)

---

## handl_lib_test::shared::b

*Function*

```rust
fn b(v: bool) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_test::shared::call_function

*Function*

```rust
fn call_function(ctx: & mut dyn Context, func: &ac_runtime_values::FunctionValue, args: &[ac_runtime_values::NonFunctionValue]) -> Result<ac_runtime_values::NonFunctionValue, ac_runtime_values::Error>
```



## handl_lib_test::shared::obj

*Function*

```rust
fn obj(values: &[(&str, serde_json::Value)]) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_test::shared::require_array

*Function*

```rust
fn require_array(value: &ac_runtime_values::NonFunctionValue) -> Result<&[serde_json::Value], ac_runtime_values::Error>
```



## handl_lib_test::shared::require_bool

*Function*

```rust
fn require_bool(value: &ac_runtime_values::NonFunctionValue) -> Result<bool, ac_runtime_values::Error>
```



## handl_lib_test::shared::require_function

*Function*

```rust
fn require_function(value: &ac_runtime_values::NonFunctionValue) -> Result<ac_runtime_values::FunctionValue, ac_runtime_values::Error>
```



## handl_lib_test::shared::require_integer

*Function*

```rust
fn require_integer(value: &ac_runtime_values::NonFunctionValue) -> Result<i64, ac_runtime_values::Error>
```



## handl_lib_test::shared::require_len

*Function*

```rust
fn require_len(args: &[ac_runtime_values::NonFunctionValue], expected: usize) -> Result<(), ac_runtime_values::Error>
```



## handl_lib_test::shared::require_number

*Function*

```rust
fn require_number(value: &ac_runtime_values::NonFunctionValue) -> Result<f64, ac_runtime_values::Error>
```



## handl_lib_test::shared::require_string

*Function*

```rust
fn require_string(value: &ac_runtime_values::NonFunctionValue) -> Result<&str, ac_runtime_values::Error>
```



## handl_lib_test::shared::str

*Function*

```rust
fn str<impl Into<String>>(v: impl Trait) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_test::shared::type_name

*Function*

```rust
fn type_name(value: &ac_runtime_values::NonFunctionValue) -> String
```



