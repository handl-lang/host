**handl_lib_collections**

# Module: handl_lib_collections

## Contents

**Modules**

- [`bit_set`](#bit_set) - `collections.bit_set` — compact set of non-negative integers stored as bits.
- [`btree_map`](#btree_map) - `collections.btree_map` — ordered key/value dictionary backed by a B-tree.
- [`btree_set`](#btree_set) - `collections.btree_set` — ordered set of unique values.
- [`common`](#common)
- [`deque`](#deque) - `collections.deque` — double-ended queue.
- [`error`](#error)
- [`hash_map`](#hash_map) - `collections.hash_map` — key/value dictionary backed by a hash table.
- [`hash_set`](#hash_set) - `collections.hash_set` — unordered set of unique values.
- [`list`](#list) - `collections.list` — front-ended list.
- [`module`](#module)
- [`ops`](#ops)
- [`priority_queue`](#priority_queue) - `collections.priority_queue` — priority queue / heap.
- [`queue`](#queue) - `collections.queue` — first-in-first-out queue.
- [`stack`](#stack) - `collections.stack` — last-in-first-out stack.
- [`vec`](#vec) - `collections.vec` — back-ended dynamic array.

---

## Module: bit_set

`collections.bit_set` — compact set of non-negative integers stored as bits.

Reserved for future implementation.



## Module: btree_map

`collections.btree_map` — ordered key/value dictionary backed by a B-tree.

Reserved for future implementation.



## Module: btree_set

`collections.btree_set` — ordered set of unique values.

Reserved for future implementation.



## Module: common



## Module: deque

`collections.deque` — double-ended queue.

Operations: `new`, `from`, `to_array`, `len`, `is_empty`,
`push_back`, `push_front`, `pop_back`, `pop_front`, `peek_back`, `peek_front`.



## Module: error



## Module: hash_map

`collections.hash_map` — key/value dictionary backed by a hash table.

Currently implemented as a wrapper around `serde_json::Value::Object`.



## Module: hash_set

`collections.hash_set` — unordered set of unique values.

Implemented as a deduplicated array of `serde_json::Value`.



## Module: list

`collections.list` — front-ended list.

Operations: `new`, `from`, `to_array`, `len`, `is_empty`, `push`, `pop`, `peek`.



## Module: module



## Module: ops



## Module: priority_queue

`collections.priority_queue` — priority queue / heap.

Reserved for future implementation.



## Module: queue

`collections.queue` — first-in-first-out queue.

Operations: `new`, `from`, `to_array`, `len`, `is_empty`, `enqueue`, `dequeue`, `peek`.



## Module: stack

`collections.stack` — last-in-first-out stack.

Operations: `new`, `from`, `to_array`, `len`, `is_empty`, `push`, `pop`, `peek`.



## Module: vec

`collections.vec` — back-ended dynamic array.

Operations: `new`, `from`, `to_array`, `len`, `is_empty`, `push`, `pop`, `peek`.



