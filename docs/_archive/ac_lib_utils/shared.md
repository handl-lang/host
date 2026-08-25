**handl_lib_utils > shared**

# Module: shared

## Contents

**Functions**

- [`apply_fn`](#apply_fn) - Apply a single-param function to a value: bind_param + eval_expr.
- [`apply_fn2`](#apply_fn2) - Apply a two-param function to two values: bind_two_params + eval_expr.
- [`arr`](#arr)
- [`b`](#b)
- [`bind_function_params`](#bind_function_params)
- [`bind_param`](#bind_param)
- [`bind_two_params`](#bind_two_params)
- [`destructure_pattern`](#destructure_pattern)
- [`filter_iter`](#filter_iter) - Filter an iterator by a predicate function, collecting matching items.
- [`find_first`](#find_first) - Find the first element matching a predicate.
- [`find_first_rev`](#find_first_rev) - Find the first matching element from a reverse iterator.
- [`find_index`](#find_index) - Find the index of the first element matching a predicate.
- [`find_index_rev`](#find_index_rev) - Find the index of the first matching element from a reverse iterator.
- [`fold_iter`](#fold_iter) - Fold an iterator with a two-param function and an initial accumulator.
- [`is_truthy_fn`](#is_truthy_fn) - Apply a predicate function and check if the result is truthy.
- [`is_truthy_value`](#is_truthy_value)
- [`key_to_string`](#key_to_string)
- [`map_iter`](#map_iter) - Map a function over an iterator, collecting results.
- [`num`](#num)
- [`pattern_binding_name`](#pattern_binding_name)
- [`require_array`](#require_array)
- [`require_function_and_array`](#require_function_and_array)
- [`require_function_value`](#require_function_value)
- [`require_functions`](#require_functions)
- [`require_one_arg`](#require_one_arg)
- [`require_one_array`](#require_one_array)
- [`require_two_arrays`](#require_two_arrays)
- [`value_to_string`](#value_to_string)

---

## handl_lib_utils::shared::apply_fn

*Function*

Apply a single-param function to a value: bind_param + eval_expr.

```rust
fn apply_fn(ctx: & mut dyn Context, fn_val: &ac_runtime_values::FunctionValue, value: &serde_json::Value) -> Result<ac_runtime_values::NonFunctionValue, ac_runtime_values::Error>
```



## handl_lib_utils::shared::apply_fn2

*Function*

Apply a two-param function to two values: bind_two_params + eval_expr.

```rust
fn apply_fn2(ctx: & mut dyn Context, fn_val: &ac_runtime_values::FunctionValue, a: &serde_json::Value, b: &serde_json::Value) -> Result<ac_runtime_values::NonFunctionValue, ac_runtime_values::Error>
```



## handl_lib_utils::shared::arr

*Function*

```rust
fn arr(items: &[serde_json::Value]) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_utils::shared::b

*Function*

```rust
fn b(value: bool) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_utils::shared::bind_function_params

*Function*

```rust
fn bind_function_params(fn_val: &ac_runtime_values::FunctionValue, args: &[serde_json::Value]) -> Result<Vec<(String, ac_runtime_values::NonFunctionValue)>, ac_runtime_values::Error>
```



## handl_lib_utils::shared::bind_param

*Function*

```rust
fn bind_param(fn_val: &ac_runtime_values::FunctionValue, value: &serde_json::Value) -> Vec<(String, ac_runtime_values::NonFunctionValue)>
```



## handl_lib_utils::shared::bind_two_params

*Function*

```rust
fn bind_two_params(fn_val: &ac_runtime_values::FunctionValue, acc: &serde_json::Value, item: &serde_json::Value) -> Vec<(String, ac_runtime_values::NonFunctionValue)>
```



## handl_lib_utils::shared::destructure_pattern

*Function*

```rust
fn destructure_pattern(pattern: &serde_json::Value, value: &serde_json::Value, out: & mut Vec<(String, serde_json::Value)>) -> Result<(), ac_runtime_values::Error>
```



## handl_lib_utils::shared::filter_iter

*Function*

Filter an iterator by a predicate function, collecting matching items.

```rust
fn filter_iter<'a, I>(ctx: & mut dyn Context, fn_val: &ac_runtime_values::FunctionValue, iter: I) -> Result<Vec<serde_json::Value>, ac_runtime_values::Error>
```



## handl_lib_utils::shared::find_first

*Function*

Find the first element matching a predicate.
Returns `None` if no match.

```rust
fn find_first<'a, I>(ctx: & mut dyn Context, fn_val: &ac_runtime_values::FunctionValue, iter: I) -> Result<Option<serde_json::Value>, ac_runtime_values::Error>
```



## handl_lib_utils::shared::find_first_rev

*Function*

Find the first matching element from a reverse iterator.
Returns `None` if no match.

```rust
fn find_first_rev<'a, I>(ctx: & mut dyn Context, fn_val: &ac_runtime_values::FunctionValue, iter: I) -> Result<Option<serde_json::Value>, ac_runtime_values::Error>
```



## handl_lib_utils::shared::find_index

*Function*

Find the index of the first element matching a predicate.
Returns `None` if no match.

```rust
fn find_index<'a, I>(ctx: & mut dyn Context, fn_val: &ac_runtime_values::FunctionValue, iter: I) -> Result<Option<usize>, ac_runtime_values::Error>
```



## handl_lib_utils::shared::find_index_rev

*Function*

Find the index of the first matching element from a reverse iterator.
The index is relative to the reversed iteration order.
Returns `None` if no match.

```rust
fn find_index_rev<'a, I>(ctx: & mut dyn Context, fn_val: &ac_runtime_values::FunctionValue, iter: I) -> Result<Option<usize>, ac_runtime_values::Error>
```



## handl_lib_utils::shared::fold_iter

*Function*

Fold an iterator with a two-param function and an initial accumulator.

```rust
fn fold_iter<'a, I>(ctx: & mut dyn Context, fn_val: &ac_runtime_values::FunctionValue, iter: I, init: serde_json::Value) -> Result<serde_json::Value, ac_runtime_values::Error>
```



## handl_lib_utils::shared::is_truthy_fn

*Function*

Apply a predicate function and check if the result is truthy.

```rust
fn is_truthy_fn(ctx: & mut dyn Context, fn_val: &ac_runtime_values::FunctionValue, value: &serde_json::Value) -> Result<bool, ac_runtime_values::Error>
```



## handl_lib_utils::shared::is_truthy_value

*Function*

```rust
fn is_truthy_value(value: &ac_runtime_values::NonFunctionValue) -> bool
```



## handl_lib_utils::shared::key_to_string

*Function*

```rust
fn key_to_string(value: &ac_runtime_values::NonFunctionValue) -> String
```



## handl_lib_utils::shared::map_iter

*Function*

Map a function over an iterator, collecting results.
Works with any iterator yielding `&serde_json::Value`.

```rust
fn map_iter<'a, I>(ctx: & mut dyn Context, fn_val: &ac_runtime_values::FunctionValue, iter: I) -> Result<Vec<serde_json::Value>, ac_runtime_values::Error>
```



## handl_lib_utils::shared::num

*Function*

```rust
fn num(value: f64) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_utils::shared::pattern_binding_name

*Function*

```rust
fn pattern_binding_name(pattern: &serde_json::Value) -> Option<&str>
```



## handl_lib_utils::shared::require_array

*Function*

```rust
fn require_array(arg: &ac_runtime_values::NonFunctionValue) -> Result<&[serde_json::Value], ac_runtime_values::Error>
```



## handl_lib_utils::shared::require_function_and_array

*Function*

```rust
fn require_function_and_array(args: &[ac_runtime_values::NonFunctionValue]) -> Result<(ac_runtime_values::FunctionValue, &[serde_json::Value]), ac_runtime_values::Error>
```



## handl_lib_utils::shared::require_function_value

*Function*

```rust
fn require_function_value(arg: &ac_runtime_values::NonFunctionValue) -> Result<ac_runtime_values::FunctionValue, ac_runtime_values::Error>
```



## handl_lib_utils::shared::require_functions

*Function*

```rust
fn require_functions(args: &[ac_runtime_values::NonFunctionValue]) -> Result<Vec<ac_runtime_values::FunctionValue>, ac_runtime_values::Error>
```



## handl_lib_utils::shared::require_one_arg

*Function*

```rust
fn require_one_arg(args: &[ac_runtime_values::NonFunctionValue]) -> Result<&ac_runtime_values::NonFunctionValue, ac_runtime_values::Error>
```



## handl_lib_utils::shared::require_one_array

*Function*

```rust
fn require_one_array(args: &[ac_runtime_values::NonFunctionValue]) -> Result<&[serde_json::Value], ac_runtime_values::Error>
```



## handl_lib_utils::shared::require_two_arrays

*Function*

```rust
fn require_two_arrays(args: &[ac_runtime_values::NonFunctionValue]) -> Result<(&[serde_json::Value], &[serde_json::Value]), ac_runtime_values::Error>
```



## handl_lib_utils::shared::value_to_string

*Function*

```rust
fn value_to_string(value: &serde_json::Value) -> String
```



