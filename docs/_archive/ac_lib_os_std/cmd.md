**handl_lib_os_std > cmd**

# Module: cmd

## Contents

**Structs**

- [`StdCmd`](#stdcmd) - Command execution backed by [`std::process::Command`].

---

## handl_lib_os_std::cmd::StdCmd

*Struct*

Command execution backed by [`std::process::Command`].

**Unit Struct**

**Trait Implementations:**

- **Cmd**
  - `fn run(self: &Self, program: &str, args: &[&str]) -> Result<CmdOutput, OsError>`
- **Default**
  - `fn default() -> StdCmd`



