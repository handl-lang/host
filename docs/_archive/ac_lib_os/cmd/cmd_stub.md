**handl_lib_os > cmd > cmd_stub**

# Module: cmd::cmd_stub

## Contents

**Structs**

- [`StubCmd`](#stubcmd) - A command executor that returns canned outputs by program name.

---

## handl_lib_os::cmd::cmd_stub::StubCmd

*Struct*

A command executor that returns canned outputs by program name.

**Fields:**
- `outputs: std::sync::Mutex<std::collections::HashMap<String, crate::cmd::CmdOutput>>`

**Methods:**

- `fn new() -> Self`
- `fn with<impl Into<String>>(program: impl Trait, output: CmdOutput) -> Self`

**Trait Implementations:**

- **Cmd**
  - `fn run(self: &Self, program: &str, _args: &[&str]) -> Result<CmdOutput, OsError>`
- **Default**
  - `fn default() -> StubCmd`



