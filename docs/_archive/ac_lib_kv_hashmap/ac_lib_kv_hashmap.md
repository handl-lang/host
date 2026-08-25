**handl_lib_kv_hashmap**

# Module: handl_lib_kv_hashmap

## Contents

**Structs**

- [`HashKeyValueStore`](#hashkeyvaluestore) - In-memory key-value store backed by [`HashMap`].

---

## handl_lib_kv_hashmap::HashKeyValueStore

*Struct*

In-memory key-value store backed by [`HashMap`].

Thread-safe via `Mutex`. For persistent storage, consider a future
sled or SQLite backend.

**Fields:**
- `data: std::sync::Mutex<std::collections::HashMap<String, String>>`

**Methods:**

- `fn new() -> Self`
- `fn with<impl Into<String>, impl Into<String>>(key: impl Trait, value: impl Trait) -> Self`

**Trait Implementations:**

- **Default**
  - `fn default() -> HashKeyValueStore`
- **KeyValueStore**
  - `fn get(self: &Self, key: &str) -> Result<Option<String>, KvError>`
  - `fn set(self: &Self, key: &str, value: &str) -> Result<(), KvError>`
  - `fn remove(self: &Self, key: &str) -> Result<(), KvError>`
  - `fn clear(self: &Self) -> Result<(), KvError>`
  - `fn has(self: &Self, key: &str) -> Result<bool, KvError>`
  - `fn size(self: &Self) -> Result<usize, KvError>`



