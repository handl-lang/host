# handl_lib_collections

# handl-lib-collections

Collection types for the handl runtime.

Provides the `collections` module with `vec`, `list`, `queue`,
`stack`, `deque`, `hash_map`, `hash_set`, `btree_map`, `btree_set`,
`priority_queue`, and `bit_set`.

Currently implemented:

- `vec.*` — back-ended dynamic array
- `list.*` — front-ended list
- `queue.*` — FIFO queue
- `stack.*` — LIFO stack
- `deque.*` — double-ended queue

## Usage

```ignore
use ac_runtime_modules::ModuleHost;
use handl_lib_collections::CollectionsModule;

let host = ModuleHost::new()
    .with(CollectionsModule);
```

## Modules

### [`handl_lib_collections`](handl_lib_collections.md)

*15 modules*

### [`common`](common.md)

*9 functions*

### [`error`](error.md)

*1 enum*

### [`hash_map`](hash_map.md)

*1 function, 11 structs*

### [`hash_set`](hash_set.md)

*1 function, 11 structs*

### [`module`](module.md)

*1 struct, 45 statics*

### [`ops`](ops.md)

*1 enum, 8 structs*

