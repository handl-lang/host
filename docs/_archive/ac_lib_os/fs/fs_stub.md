**handl_lib_os > fs > fs_stub**

# Module: fs::fs_stub

## Contents

**Structs**

- [`StubFs`](#stubfs) - A simple in-memory filesystem for tests.

---

## handl_lib_os::fs::fs_stub::StubFs

*Struct*

A simple in-memory filesystem for tests.

Stores file contents in a `HashMap<String, String>`. Directories are
implicit — `list_dir` returns files whose path starts with the given
prefix.

**Fields:**
- `files: std::sync::Mutex<std::collections::HashMap<String, String>>`

**Methods:**

- `fn new() -> Self`
- `fn with_file<impl Into<String>, impl Into<String>>(path: impl Trait, content: impl Trait) -> Self`

**Trait Implementations:**

- **Default**
  - `fn default() -> StubFs`
- **Fs**
  - `fn read_file(self: &Self, path: &str) -> Result<String, OsError>`
  - `fn write_file(self: &Self, path: &str, content: &str) -> Result<(), OsError>`
  - `fn exists(self: &Self, path: &str) -> Result<bool, OsError>`
  - `fn remove(self: &Self, path: &str) -> Result<(), OsError>`
  - `fn list_dir(self: &Self, prefix: &str) -> Result<Vec<String>, OsError>`



