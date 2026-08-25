# handl_lib_kv

# handl-lib-kv

Key-value store abstraction for the handl runtime.

Defines the [`KeyValueStore`] trait, [`KeyValueStoreModule`] adapter, and
[`KvError`]. The `kv.*` functions are callable from handl expressions:
`kv.get("key")`, `kv.set("key", "value")`, etc.

Unlike platform services (filesystem, env, http), a key-value store is a
stateful data abstraction — not tied to any OS capability. Backends may
include in-memory HashMaps, sled, SQLite, Redis, etc.

- [`handl_lib_kv_hashmap`] — in-memory `HashMap` backend

## Usage

```ignore
use ac_runtime_modules::ModuleHost;
use handl_lib_kv::KeyValueStoreModule;
use handl_lib_kv_hashmap::HashKeyValueStore;

let host = ModuleHost::new()
    .with(KeyValueStoreModule::new(HashKeyValueStore::default()));
```

## Modules

### [`handl_lib_kv`](handl_lib_kv.md)

*2 modules*

### [`error`](error.md)

*1 enum*

### [`key_value_store`](key_value_store.md)

*1 trait, 2 functions, 7 structs*

