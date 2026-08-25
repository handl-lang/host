**handl_lib_os > env > env_stub**

# Module: env::env_stub

## Contents

**Structs**

- [`StubEnv`](#stubenv) - A simple in-memory environment for tests.

---

## handl_lib_os::env::env_stub::StubEnv

*Struct*

A simple in-memory environment for tests.

**Fields:**
- `vars: std::sync::Mutex<std::collections::HashMap<String, String>>`

**Methods:**

- `fn new() -> Self`
- `fn with<impl Into<String>, impl Into<String>>(key: impl Trait, value: impl Trait) -> Self`

**Trait Implementations:**

- **Default**
  - `fn default() -> StubEnv`
- **Env**
  - `fn get(self: &Self, key: &str) -> Result<Option<String>, OsError>`
  - `fn has(self: &Self, key: &str) -> Result<bool, OsError>`
  - `fn keys(self: &Self) -> Result<Vec<String>, OsError>`



