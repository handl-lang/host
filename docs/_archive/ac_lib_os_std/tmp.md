**handl_lib_os_std > tmp**

# Module: tmp

## Contents

**Structs**

- [`StdTmp`](#stdtmp) - Temporary file/directory implementation backed by `std::env::temp_dir`.

**Functions**

- [`unique_suffix`](#unique_suffix)

---

## handl_lib_os_std::tmp::StdTmp

*Struct*

Temporary file/directory implementation backed by `std::env::temp_dir`.

**Unit Struct**

**Trait Implementations:**

- **Tmp**
  - `fn file(self: &Self) -> Result<String, OsError>`
  - `fn dir(self: &Self) -> Result<String, OsError>`
- **Default**
  - `fn default() -> StdTmp`



## handl_lib_os_std::tmp::unique_suffix

*Function*

```rust
fn unique_suffix() -> String
```



