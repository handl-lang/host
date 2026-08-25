**handl_lib_os_std > path**

# Module: path

## Contents

**Structs**

- [`StdPath`](#stdpath) - Path manipulation backed by [`std::path`].

---

## handl_lib_os_std::path::StdPath

*Struct*

Path manipulation backed by [`std::path`].

**Unit Struct**

**Trait Implementations:**

- **Default**
  - `fn default() -> StdPath`
- **Path**
  - `fn join(self: &Self, parts: &[&str]) -> Result<String, OsError>`
  - `fn resolve(self: &Self, parts: &[&str]) -> Result<String, OsError>`
  - `fn basename(self: &Self, path: &str) -> Result<String, OsError>`
  - `fn dirname(self: &Self, path: &str) -> Result<String, OsError>`
  - `fn extname(self: &Self, path: &str) -> Result<String, OsError>`



