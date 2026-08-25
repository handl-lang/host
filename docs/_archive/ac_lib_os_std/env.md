**handl_lib_os_std > env**

# Module: env

## Contents

**Structs**

- [`StdEnv`](#stdenv) - Environment variable implementation backed by [`std::env`].

---

## handl_lib_os_std::env::StdEnv

*Struct*

Environment variable implementation backed by [`std::env`].

**Unit Struct**

**Trait Implementations:**

- **Env**
  - `fn get(self: &Self, key: &str) -> Result<Option<String>, OsError>`
  - `fn has(self: &Self, key: &str) -> Result<bool, OsError>`
  - `fn keys(self: &Self) -> Result<Vec<String>, OsError>`
- **Default**
  - `fn default() -> StdEnv`



