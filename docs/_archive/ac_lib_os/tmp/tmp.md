**handl_lib_os > tmp > tmp**

# Module: tmp::tmp

## Contents

**Structs**

- [`DirFn`](#dirfn)
- [`FileFn`](#filefn)
- [`TmpModule`](#tmpmodule) - Module adapter for [`Tmp`] — exposes `tmp.*` functions.

**Traits**

- [`Tmp`](#tmp) - Abstract temporary file/directory service.

---

## handl_lib_os::tmp::tmp::DirFn

*Struct*

**Fields:**
- `tmp: std::sync::Arc<dyn Tmp>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, _args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::tmp::tmp::FileFn

*Struct*

**Fields:**
- `tmp: std::sync::Arc<dyn Tmp>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, _args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::tmp::tmp::Tmp

*Trait*

Abstract temporary file/directory service.

Implementations:
- `handl-lib-os-std` — backed by `std::env::temp_dir`
- [`tmp_stub::StubTmp`] — canned paths for testing

**Methods:**

- `file`
- `dir`



## handl_lib_os::tmp::tmp::TmpModule

*Struct*

Module adapter for [`Tmp`] — exposes `tmp.*` functions.

**Fields:**
- `file: FileFn`
- `dir: DirFn`

**Methods:**

- `fn new<impl Tmp + 'static>(tmp: impl Trait) -> Self`

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`



