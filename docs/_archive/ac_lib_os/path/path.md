**handl_lib_os > path > path**

# Module: path::path

## Contents

**Structs**

- [`BasenameFn`](#basenamefn)
- [`DirnameFn`](#dirnamefn)
- [`ExtnameFn`](#extnamefn)
- [`JoinFn`](#joinfn)
- [`PathModule`](#pathmodule) - Module adapter for [`Path`] — exposes `path.*` functions.
- [`ResolveFn`](#resolvefn)

**Functions**

- [`collect_string_args`](#collect_string_args)
- [`require_one_string`](#require_one_string)

**Traits**

- [`Path`](#path) - Abstract path manipulation service.

---

## handl_lib_os::path::path::BasenameFn

*Struct*

**Fields:**
- `path: std::sync::Arc<dyn Path>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::path::path::DirnameFn

*Struct*

**Fields:**
- `path: std::sync::Arc<dyn Path>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::path::path::ExtnameFn

*Struct*

**Fields:**
- `path: std::sync::Arc<dyn Path>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::path::path::JoinFn

*Struct*

**Fields:**
- `path: std::sync::Arc<dyn Path>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::path::path::Path

*Trait*

Abstract path manipulation service.

Implementations:
- `handl-lib-os-std` — backed by `std::path`
- [`path_stub::StubPath`] — POSIX-like for testing

**Methods:**

- `join`
- `resolve`
- `basename`
- `dirname`
- `extname`



## handl_lib_os::path::path::PathModule

*Struct*

Module adapter for [`Path`] — exposes `path.*` functions.

**Fields:**
- `join: JoinFn`
- `resolve: ResolveFn`
- `basename: BasenameFn`
- `dirname: DirnameFn`
- `extname: ExtnameFn`

**Methods:**

- `fn new<impl Path + 'static>(path: impl Trait) -> Self`

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`



## handl_lib_os::path::path::ResolveFn

*Struct*

**Fields:**
- `path: std::sync::Arc<dyn Path>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::path::path::collect_string_args

*Function*

```rust
fn collect_string_args(args: &[ac_runtime_values::NonFunctionValue]) -> Result<Vec<&str>, ac_runtime_values::Error>
```



## handl_lib_os::path::path::require_one_string

*Function*

```rust
fn require_one_string(args: &[ac_runtime_values::NonFunctionValue]) -> Result<&str, ac_runtime_values::Error>
```



