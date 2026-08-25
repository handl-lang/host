**handl_lib_os > path > path_stub**

# Module: path::path_stub

## Contents

**Structs**

- [`StubPath`](#stubpath) - A simple POSIX-like path implementation for tests.

---

## handl_lib_os::path::path_stub::StubPath

*Struct*

A simple POSIX-like path implementation for tests.

**Unit Struct**

**Methods:**

- `fn new() -> Self`

**Trait Implementations:**

- **Path**
  - `fn join(self: &Self, parts: &[&str]) -> Result<String, OsError>`
  - `fn resolve(self: &Self, parts: &[&str]) -> Result<String, OsError>`
  - `fn basename(self: &Self, path: &str) -> Result<String, OsError>`
  - `fn dirname(self: &Self, path: &str) -> Result<String, OsError>`
  - `fn extname(self: &Self, path: &str) -> Result<String, OsError>`
- **Default**
  - `fn default() -> StubPath`



