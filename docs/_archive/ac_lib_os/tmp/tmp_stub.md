**handl_lib_os > tmp > tmp_stub**

# Module: tmp::tmp_stub

## Contents

**Structs**

- [`StubTmp`](#stubtmp) - A stub that returns sequential temporary paths.

---

## handl_lib_os::tmp::tmp_stub::StubTmp

*Struct*

A stub that returns sequential temporary paths.

**Fields:**
- `counter: std::sync::atomic::AtomicUsize`

**Trait Implementations:**

- **Tmp**
  - `fn file(self: &Self) -> Result<String, OsError>`
  - `fn dir(self: &Self) -> Result<String, OsError>`
- **Default**
  - `fn default() -> StubTmp`



