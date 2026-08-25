**handl_lib_os > process > process**

# Module: process::process

## Contents

**Structs**

- [`KillFn`](#killfn)
- [`ProcessModule`](#processmodule) - Module adapter for [`Process`] — exposes `process.*` functions.
- [`ProcessStatus`](#processstatus) - Snapshot of a managed child process.
- [`SpawnFn`](#spawnfn)
- [`StatusFn`](#statusfn)
- [`WaitFn`](#waitfn)

**Functions**

- [`status_to_value`](#status_to_value)

**Traits**

- [`Process`](#process) - Abstract child-process management service.

---

## handl_lib_os::process::process::KillFn

*Struct*

**Fields:**
- `process: std::sync::Arc<dyn Process>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::process::process::Process

*Trait*

Abstract child-process management service.

Provides lifecycle control over OS processes: spawn, kill, wait, and poll.

Implementations:
- `handl-lib-os-std` — backed by `std::process`
- [`process_stub::StubProcess`] — canned process lifecycle for testing

**Methods:**

- `spawn`
- `kill`
- `wait`
- `status`



## handl_lib_os::process::process::ProcessModule

*Struct*

Module adapter for [`Process`] — exposes `process.*` functions.

**Fields:**
- `spawn: SpawnFn`
- `kill: KillFn`
- `wait: WaitFn`
- `status: StatusFn`

**Methods:**

- `fn new<impl Process + 'static>(process: impl Trait) -> Self`

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`



## handl_lib_os::process::process::ProcessStatus

*Struct*

Snapshot of a managed child process.

**Fields:**
- `process_id: i64`
- `running: bool`
- `exit_code: Option<i32>`

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> ProcessStatus`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_lib_os::process::process::SpawnFn

*Struct*

**Fields:**
- `process: std::sync::Arc<dyn Process>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::process::process::StatusFn

*Struct*

**Fields:**
- `process: std::sync::Arc<dyn Process>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::process::process::WaitFn

*Struct*

**Fields:**
- `process: std::sync::Arc<dyn Process>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::process::process::status_to_value

*Function*

```rust
fn status_to_value(status: ProcessStatus) -> ac_runtime_values::NonFunctionValue
```



