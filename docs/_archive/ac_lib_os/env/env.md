**handl_lib_os > env > env**

# Module: env::env

## Contents

**Structs**

- [`EnvModule`](#envmodule) - Module adapter for [`Env`] — exposes `env.*` functions.
- [`GetFn`](#getfn)
- [`HasFn`](#hasfn)
- [`KeysFn`](#keysfn)

**Functions**

- [`require_one_string`](#require_one_string)

**Traits**

- [`Env`](#env) - Abstract environment variable service.

---

## handl_lib_os::env::env::Env

*Trait*

Abstract environment variable service.

Implementations:
- `handl-lib-os-std` — backed by `std::env`
- [`env_stub::StubEnv`] — in-memory for testing

**Methods:**

- `get`
- `has`
- `keys`



## handl_lib_os::env::env::EnvModule

*Struct*

Module adapter for [`Env`] — exposes `env.*` functions.

**Fields:**
- `get: GetFn`
- `has: HasFn`
- `keys: KeysFn`

**Methods:**

- `fn new<impl Env + 'static>(env: impl Trait) -> Self`

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`



## handl_lib_os::env::env::GetFn

*Struct*

**Fields:**
- `env: std::sync::Arc<dyn Env>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::env::env::HasFn

*Struct*

**Fields:**
- `env: std::sync::Arc<dyn Env>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::env::env::KeysFn

*Struct*

**Fields:**
- `env: std::sync::Arc<dyn Env>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, _args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::env::env::require_one_string

*Function*

```rust
fn require_one_string(args: &[ac_runtime_values::NonFunctionValue]) -> Result<&str, ac_runtime_values::Error>
```



