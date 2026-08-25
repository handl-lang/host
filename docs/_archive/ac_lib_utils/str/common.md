**handl_lib_utils > str > common**

# Module: str::common

## Contents

**Macros**

- [`string_predicate_fn`](#string_predicate_fn)
- [`string_unary_fn`](#string_unary_fn)

**Functions**

- [`b`](#b)
- [`num`](#num)
- [`require_one_string`](#require_one_string)
- [`require_string`](#require_string)
- [`require_two_strings`](#require_two_strings)
- [`s`](#s)

---

## handl_lib_utils::str::common::b

*Function*

```rust
fn b(value: bool) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_utils::str::common::num

*Function*

```rust
fn num(value: f64) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_utils::str::common::require_one_string

*Function*

```rust
fn require_one_string(args: &[ac_runtime_values::NonFunctionValue]) -> Result<&str, ac_runtime_values::Error>
```



## handl_lib_utils::str::common::require_string

*Function*

```rust
fn require_string(arg: &ac_runtime_values::NonFunctionValue) -> Result<&str, ac_runtime_values::Error>
```



## handl_lib_utils::str::common::require_two_strings

*Function*

```rust
fn require_two_strings(args: &[ac_runtime_values::NonFunctionValue]) -> Result<(&str, &str), ac_runtime_values::Error>
```



## handl_lib_utils::str::common::s

*Function*

```rust
fn s<impl Into<String>>(value: impl Trait) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_utils::str::common::string_predicate_fn

*Declarative Macro*

```rust
macro_rules! string_predicate_fn {
    ($struct_name:ident, $body:expr) => { ... };
}
```



## handl_lib_utils::str::common::string_unary_fn

*Declarative Macro*

```rust
macro_rules! string_unary_fn {
    ($struct_name:ident, $body:expr) => { ... };
}
```



