**handl_lib_os_std > fs**

# Module: fs

## Contents

**Structs**

- [`StdFs`](#stdfs) - Filesystem implementation backed by [`std::fs`].

---

## handl_lib_os_std::fs::StdFs

*Struct*

Filesystem implementation backed by [`std::fs`].

**Unit Struct**

**Trait Implementations:**

- **Default**
  - `fn default() -> StdFs`
- **Fs**
  - `fn read_file(self: &Self, path: &str) -> Result<String, OsError>`
  - `fn write_file(self: &Self, path: &str, content: &str) -> Result<(), OsError>`
  - `fn exists(self: &Self, path: &str) -> Result<bool, OsError>`
  - `fn remove(self: &Self, path: &str) -> Result<(), OsError>`
  - `fn list_dir(self: &Self, path: &str) -> Result<Vec<String>, OsError>`



