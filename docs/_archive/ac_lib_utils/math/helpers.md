**handl_lib_utils > math > helpers**

# Module: math::helpers

## Contents

**Macros**

- [`bool_unary_fn`](#bool_unary_fn)
- [`unary_fn`](#unary_fn)

**Functions**

- [`require_number`](#require_number)
- [`require_two_numbers`](#require_two_numbers)

---

## handl_lib_utils::math::helpers::bool_unary_fn

*Declarative Macro*

```rust
macro_rules! bool_unary_fn {
    ($struct_name:ident, $body:expr) => { ... };
}
```



## handl_lib_utils::math::helpers::require_number

*Function*

```rust
fn require_number(arg: &ac_runtime_values::NonFunctionValue) -> Result<f64, ac_runtime_values::Error>
```



## handl_lib_utils::math::helpers::require_two_numbers

*Function*

```rust
fn require_two_numbers(args: &[ac_runtime_values::NonFunctionValue]) -> Result<(f64, f64), ac_runtime_values::Error>
```



## handl_lib_utils::math::helpers::unary_fn

*Declarative Macro*

```rust
macro_rules! unary_fn {
    ($struct_name:ident, $body:expr) => { ... };
}
```



