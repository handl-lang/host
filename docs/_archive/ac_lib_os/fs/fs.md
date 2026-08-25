**handl_lib_os > fs > fs**

# Module: fs::fs

## Contents

**Structs**

- [`ExistsFn`](#existsfn)
- [`FsModule`](#fsmodule) - Module adapter for [`Fs`] — exposes `fs.*` functions.
- [`ListDirFn`](#listdirfn)
- [`ReadFileFn`](#readfilefn)
- [`RemoveFn`](#removefn)
- [`WriteFileFn`](#writefilefn)

**Functions**

- [`require_string_arg`](#require_string_arg)

**Traits**

- [`Fs`](#fs) - Abstract filesystem service.

---

## handl_lib_os::fs::fs::ExistsFn

*Struct*

**Fields:**
- `fs: std::sync::Arc<dyn Fs>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::fs::fs::Fs

*Trait*

Abstract filesystem service.

Defines file I/O operations without tying code to a concrete platform.
Implementations:
- `handl-lib-os-std` — backed by `std::fs`
- [`fs_stub::StubFs`] — in-memory for testing

**Methods:**

- `read_file`
- `write_file`
- `exists`
- `remove`
- `list_dir`



## handl_lib_os::fs::fs::FsModule

*Struct*

Module adapter for [`Fs`] — exposes `fs.*` functions.

**Fields:**
- `read: ReadFileFn`
- `write: WriteFileFn`
- `exists: ExistsFn`
- `remove: RemoveFn`
- `list: ListDirFn`

**Methods:**

- `fn new<impl Fs + 'static>(fs: impl Trait) -> Self`

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`



## handl_lib_os::fs::fs::ListDirFn

*Struct*

**Fields:**
- `fs: std::sync::Arc<dyn Fs>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::fs::fs::ReadFileFn

*Struct*

**Fields:**
- `fs: std::sync::Arc<dyn Fs>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::fs::fs::RemoveFn

*Struct*

**Fields:**
- `fs: std::sync::Arc<dyn Fs>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::fs::fs::WriteFileFn

*Struct*

**Fields:**
- `fs: std::sync::Arc<dyn Fs>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::fs::fs::require_string_arg

*Function*

```rust
fn require_string_arg(args: &[ac_runtime_values::NonFunctionValue], expected: usize) -> Result<Vec<&str>, ac_runtime_values::Error>
```



