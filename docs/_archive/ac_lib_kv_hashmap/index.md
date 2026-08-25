# handl_lib_kv_hashmap

# handl-lib-kv-hashmap

In-memory [`HashMap`]-backed [`KeyValueStore`] implementation.

Provides [`HashKeyValueStore`] — a thread-safe, in-memory key-value store
suitable for testing, development, and ephemeral workloads.

## Usage

```ignore
use ac_runtime_modules::ModuleHost;
use handl_lib_kv::{KeyValueStoreModule};
use handl_lib_kv_hashmap::HashKeyValueStore;

let host = ModuleHost::new()
    .with(KeyValueStoreModule::new(HashKeyValueStore::default()));
```

## Modules

### [`handl_lib_kv_hashmap`](handl_lib_kv_hashmap.md)

*1 struct*

