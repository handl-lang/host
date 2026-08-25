**handl_lib_datetime > datetime > common**

# Module: datetime::common

## Contents

**Functions**

- [`b`](#b)
- [`format_iso`](#format_iso)
- [`n`](#n)
- [`parse_iso`](#parse_iso)
- [`parse_offset`](#parse_offset)
- [`parse_with_format_and_offset`](#parse_with_format_and_offset)
- [`require_number`](#require_number)
- [`require_one_string`](#require_one_string)
- [`require_string`](#require_string)
- [`require_string_and_number`](#require_string_and_number)
- [`require_three_strings`](#require_three_strings)
- [`require_two_strings`](#require_two_strings)
- [`s`](#s)

---

## handl_lib_datetime::datetime::common::b

*Function*

```rust
fn b(value: bool) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_datetime::datetime::common::format_iso

*Function*

```rust
fn format_iso(dt: &chrono::DateTime<chrono::Utc>) -> String
```



## handl_lib_datetime::datetime::common::n

*Function*

```rust
fn n(value: f64) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_datetime::datetime::common::parse_iso

*Function*

```rust
fn parse_iso(s: &str) -> Result<chrono::DateTime<chrono::Utc>, ac_runtime_values::Error>
```



## handl_lib_datetime::datetime::common::parse_offset

*Function*

```rust
fn parse_offset(offset_str: &str) -> Result<chrono::FixedOffset, ac_runtime_values::Error>
```



## handl_lib_datetime::datetime::common::parse_with_format_and_offset

*Function*

```rust
fn parse_with_format_and_offset(s: &str, fmt: &str, offset: Option<&str>) -> Result<chrono::DateTime<chrono::Utc>, ac_runtime_values::Error>
```



## handl_lib_datetime::datetime::common::require_number

*Function*

```rust
fn require_number(arg: &ac_runtime_values::NonFunctionValue) -> Result<f64, ac_runtime_values::Error>
```



## handl_lib_datetime::datetime::common::require_one_string

*Function*

```rust
fn require_one_string(args: &[ac_runtime_values::NonFunctionValue]) -> Result<&str, ac_runtime_values::Error>
```



## handl_lib_datetime::datetime::common::require_string

*Function*

```rust
fn require_string(arg: &ac_runtime_values::NonFunctionValue) -> Result<&str, ac_runtime_values::Error>
```



## handl_lib_datetime::datetime::common::require_string_and_number

*Function*

```rust
fn require_string_and_number(args: &[ac_runtime_values::NonFunctionValue]) -> Result<(&str, f64), ac_runtime_values::Error>
```



## handl_lib_datetime::datetime::common::require_three_strings

*Function*

```rust
fn require_three_strings(args: &[ac_runtime_values::NonFunctionValue]) -> Result<(&str, &str, &str), ac_runtime_values::Error>
```



## handl_lib_datetime::datetime::common::require_two_strings

*Function*

```rust
fn require_two_strings(args: &[ac_runtime_values::NonFunctionValue]) -> Result<(&str, &str), ac_runtime_values::Error>
```



## handl_lib_datetime::datetime::common::s

*Function*

```rust
fn s<impl Into<String>>(value: impl Trait) -> ac_runtime_values::NonFunctionValue
```



