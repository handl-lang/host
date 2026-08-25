**handl_lib_os_std > process**

# Module: process

## Contents

**Structs**

- [`StdProcess`](#stdprocess) - Process management backed by [`std::process`].

**Functions**

- [`exit_code`](#exit_code)

---

## handl_lib_os_std::process::StdProcess

*Struct*

Process management backed by [`std::process`].

**Fields:**
- `processes: std::sync::Mutex<std::collections::HashMap<i64, std::process::Child>>`

**Trait Implementations:**

- **Process**
  - `fn spawn(self: &Self, program: &str, args: &[&str]) -> Result<i64, OsError>`
  - `fn kill(self: &Self, process_id: i64) -> Result<(), OsError>`
  - `fn wait(self: &Self, process_id: i64) -> Result<i32, OsError>`
  - `fn status(self: &Self, process_id: i64) -> Result<ProcessStatus, OsError>`
- **Default**
  - `fn default() -> StdProcess`



## handl_lib_os_std::process::exit_code

*Function*

```rust
fn exit_code(status: std::process::ExitStatus) -> i32
```



