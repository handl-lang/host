**handl_lib_os > cmd > cmd**

# Module: cmd::cmd

## Contents

**Structs**

- [`CmdModule`](#cmdmodule) - Module adapter for [`Cmd`] — exposes `cmd.*` functions.
- [`CmdOutput`](#cmdoutput) - Output from a command execution.
- [`RunFn`](#runfn)

**Functions**

- [`output_to_value`](#output_to_value)

**Traits**

- [`Cmd`](#cmd) - Abstract command execution service.

---

## handl_lib_os::cmd::cmd::Cmd

*Trait*

Abstract command execution service.

Provides a way to interact with the command line — spawn processes,
capture stdout/stderr, and check exit codes.

Implementations:
- `handl-lib-os-std` — backed by `std::process::Command`
- [`cmd_stub::StubCmd`] — canned outputs for testing

**Methods:**

- `run`



## handl_lib_os::cmd::cmd::CmdModule

*Struct*

Module adapter for [`Cmd`] — exposes `cmd.*` functions.

**Fields:**
- `run: RunFn`

**Methods:**

- `fn new<impl Cmd + 'static>(command: impl Trait) -> Self`

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`



## handl_lib_os::cmd::cmd::CmdOutput

*Struct*

Output from a command execution.

**Fields:**
- `exit_code: i32`
- `stdout: String`
- `stderr: String`

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> CmdOutput`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_lib_os::cmd::cmd::RunFn

*Struct*

**Fields:**
- `command: std::sync::Arc<dyn Cmd>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_os::cmd::cmd::output_to_value

*Function*

```rust
fn output_to_value(output: CmdOutput) -> ac_runtime_values::NonFunctionValue
```



