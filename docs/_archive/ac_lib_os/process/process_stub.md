**handl_lib_os > process > process_stub**

# Module: process::process_stub

## Contents

**Structs**

- [`Child`](#child)
- [`StubProcess`](#stubprocess) - A process service with a controllable in-memory process table.

---

## handl_lib_os::process::process_stub::Child

*Struct*

**Fields:**
- `exit_code: i32`
- `running: bool`
- `killed: bool`

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Child`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_lib_os::process::process_stub::StubProcess

*Struct*

A process service with a controllable in-memory process table.

**Fields:**
- `next_id: std::sync::atomic::AtomicI64`
- `processes: std::sync::Mutex<std::collections::HashMap<i64, Child>>`

**Methods:**

- `fn new() -> Self`
- `fn with<impl Into<String>>(process_id: i64, _program: impl Trait, _args: Vec<String>, exit_code: i32, running: bool) -> Self`

**Trait Implementations:**

- **Default**
  - `fn default() -> Self`
- **Process**
  - `fn spawn(self: &Self, _program: &str, _args: &[&str]) -> Result<i64, OsError>`
  - `fn kill(self: &Self, process_id: i64) -> Result<(), OsError>`
  - `fn wait(self: &Self, process_id: i64) -> Result<i32, OsError>`
  - `fn status(self: &Self, process_id: i64) -> Result<ProcessStatus, OsError>`



