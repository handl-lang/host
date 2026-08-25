**handl_lib_crypto > common**

# Module: common

## Contents

**Functions**

- [`byte_array`](#byte_array)
- [`extract_bytes`](#extract_bytes) - Extract bytes from an argument that is either a string (UTF-8) or an
- [`require_string`](#require_string) - Require a string argument at `idx`.
- [`s`](#s)

---

## handl_lib_crypto::common::byte_array

*Function*

```rust
fn byte_array(bytes: &[u8]) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_crypto::common::extract_bytes

*Function*

Extract bytes from an argument that is either a string (UTF-8) or an
array of integers in the range 0–255.

```rust
fn extract_bytes(args: &[ac_runtime_values::NonFunctionValue], idx: usize) -> Result<Vec<u8>, ac_runtime_values::Error>
```



## handl_lib_crypto::common::require_string

*Function*

Require a string argument at `idx`.

```rust
fn require_string(args: &[ac_runtime_values::NonFunctionValue], idx: usize) -> Result<String, ac_runtime_values::Error>
```



## handl_lib_crypto::common::s

*Function*

```rust
fn s<impl Into<String>>(value: impl Trait) -> ac_runtime_values::NonFunctionValue
```



