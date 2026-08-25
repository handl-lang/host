**handl_lib_kv > key_value_store**

# Module: key_value_store

## Contents

**Structs**

- [`ClearFn`](#clearfn)
- [`GetFn`](#getfn)
- [`HasFn`](#hasfn)
- [`KeyValueStoreModule`](#keyvaluestoremodule) - Module adapter for [`KeyValueStore`] — exposes `kv.*` functions.
- [`RemoveFn`](#removefn)
- [`SetFn`](#setfn)
- [`SizeFn`](#sizefn)

**Functions**

- [`require_one_string`](#require_one_string)
- [`require_two_strings`](#require_two_strings)

**Traits**

- [`KeyValueStore`](#keyvaluestore) - Abstract key-value store service.

---

## handl_lib_kv::key_value_store::ClearFn

*Struct*

**Fields:**
- `store: std::sync::Arc<dyn KeyValueStore>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, _args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_kv::key_value_store::GetFn

*Struct*

**Fields:**
- `store: std::sync::Arc<dyn KeyValueStore>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_kv::key_value_store::HasFn

*Struct*

**Fields:**
- `store: std::sync::Arc<dyn KeyValueStore>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_kv::key_value_store::KeyValueStore

*Trait*

Abstract key-value store service.

Implementations:
- [`crate::StubKeyValueStore`] — in-memory `HashMap` for testing
- Future backends: sled, SQLite, Redis, etc.

**Methods:**

- `get`
- `set`
- `remove`
- `clear`
- `has`
- `size`



## handl_lib_kv::key_value_store::KeyValueStoreModule

*Struct*

Module adapter for [`KeyValueStore`] — exposes `kv.*` functions.

**Fields:**
- `get: GetFn`
- `set: SetFn`
- `remove: RemoveFn`
- `clear: ClearFn`
- `has: HasFn`
- `size: SizeFn`

**Methods:**

- `fn new<impl KeyValueStore + 'static>(store: impl Trait) -> Self`

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`



## handl_lib_kv::key_value_store::RemoveFn

*Struct*

**Fields:**
- `store: std::sync::Arc<dyn KeyValueStore>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_kv::key_value_store::SetFn

*Struct*

**Fields:**
- `store: std::sync::Arc<dyn KeyValueStore>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_kv::key_value_store::SizeFn

*Struct*

**Fields:**
- `store: std::sync::Arc<dyn KeyValueStore>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, _args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_kv::key_value_store::require_one_string

*Function*

```rust
fn require_one_string(args: &[ac_runtime_values::NonFunctionValue]) -> Result<&str, ac_runtime_values::Error>
```



## handl_lib_kv::key_value_store::require_two_strings

*Function*

```rust
fn require_two_strings(args: &[ac_runtime_values::NonFunctionValue]) -> Result<(&str, &str), ac_runtime_values::Error>
```



