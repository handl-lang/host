**handl_lib_os > time > time_stub**

# Module: time::time_stub

## Contents

**Structs**

- [`StubTime`](#stubtime) - A time service that always returns the same fixed time.

---

## handl_lib_os::time::time_stub::StubTime

*Struct*

A time service that always returns the same fixed time.

**Fields:**
- `millis: i64`
- `seconds: i64`

**Methods:**

- `fn new(millis: i64, seconds: i64) -> Self`

**Trait Implementations:**

- **Default**
  - `fn default() -> Self`
- **Time**
  - `fn now_millis(self: &Self) -> Result<i64, OsError>`
  - `fn now_seconds(self: &Self) -> Result<i64, OsError>`



