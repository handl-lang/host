**handl_lib_os_std > time**

# Module: time

## Contents

**Structs**

- [`StdTime`](#stdtime) - Time implementation backed by [`std::time::SystemTime`].

---

## handl_lib_os_std::time::StdTime

*Struct*

Time implementation backed by [`std::time::SystemTime`].

**Unit Struct**

**Trait Implementations:**

- **Time**
  - `fn now_millis(self: &Self) -> Result<i64, OsError>`
  - `fn now_seconds(self: &Self) -> Result<i64, OsError>`
- **Default**
  - `fn default() -> StdTime`



