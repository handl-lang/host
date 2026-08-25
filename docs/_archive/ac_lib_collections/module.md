**handl_lib_collections > module**

# Module: module

## Contents

**Structs**

- [`CollectionsModule`](#collectionsmodule) - The `collections` module — collection types for the handl runtime.

**Statics**

- [`DEQUE_PEEK_BACK`](#deque_peek_back)
- [`DEQUE_PEEK_FRONT`](#deque_peek_front)
- [`DEQUE_POP_BACK`](#deque_pop_back)
- [`DEQUE_POP_FRONT`](#deque_pop_front)
- [`DEQUE_PUSH_BACK`](#deque_push_back)
- [`DEQUE_PUSH_FRONT`](#deque_push_front)
- [`FROM`](#from)
- [`HASH_MAP_FROM`](#hash_map_from)
- [`HASH_MAP_GET`](#hash_map_get)
- [`HASH_MAP_HAS`](#hash_map_has)
- [`HASH_MAP_IS_EMPTY`](#hash_map_is_empty)
- [`HASH_MAP_KEYS`](#hash_map_keys)
- [`HASH_MAP_LEN`](#hash_map_len)
- [`HASH_MAP_NEW`](#hash_map_new)
- [`HASH_MAP_REMOVE`](#hash_map_remove)
- [`HASH_MAP_SET`](#hash_map_set)
- [`HASH_MAP_TO_OBJECT`](#hash_map_to_object)
- [`HASH_MAP_VALUES`](#hash_map_values)
- [`HASH_SET_ADD`](#hash_set_add)
- [`HASH_SET_DIFFERENCE`](#hash_set_difference)
- [`HASH_SET_FROM`](#hash_set_from)
- [`HASH_SET_HAS`](#hash_set_has)
- [`HASH_SET_INTERSECTION`](#hash_set_intersection)
- [`HASH_SET_IS_EMPTY`](#hash_set_is_empty)
- [`HASH_SET_LEN`](#hash_set_len)
- [`HASH_SET_NEW`](#hash_set_new)
- [`HASH_SET_REMOVE`](#hash_set_remove)
- [`HASH_SET_TO_ARRAY`](#hash_set_to_array)
- [`HASH_SET_UNION`](#hash_set_union)
- [`IS_EMPTY`](#is_empty)
- [`LEN`](#len)
- [`LIST_PEEK`](#list_peek)
- [`LIST_POP`](#list_pop)
- [`LIST_PUSH`](#list_push)
- [`NEW`](#new)
- [`QUEUE_DEQUEUE`](#queue_dequeue)
- [`QUEUE_ENQUEUE`](#queue_enqueue)
- [`QUEUE_PEEK`](#queue_peek)
- [`STACK_PEEK`](#stack_peek)
- [`STACK_POP`](#stack_pop)
- [`STACK_PUSH`](#stack_push)
- [`TO_ARRAY`](#to_array)
- [`VEC_PEEK`](#vec_peek)
- [`VEC_POP`](#vec_pop)
- [`VEC_PUSH`](#vec_push)

---

## handl_lib_collections::module::CollectionsModule

*Struct*

The `collections` module — collection types for the handl runtime.

Currently implemented: `vec`, `list`, `queue`, `stack`, `deque`, `hash_map`, `hash_set`.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
- **Clone**
  - `fn clone(self: &Self) -> CollectionsModule`



## handl_lib_collections::module::DEQUE_PEEK_BACK

*Static*

```rust
static DEQUE_PEEK_BACK: super::ops::Peek
```



## handl_lib_collections::module::DEQUE_PEEK_FRONT

*Static*

```rust
static DEQUE_PEEK_FRONT: super::ops::Peek
```



## handl_lib_collections::module::DEQUE_POP_BACK

*Static*

```rust
static DEQUE_POP_BACK: super::ops::Pop
```



## handl_lib_collections::module::DEQUE_POP_FRONT

*Static*

```rust
static DEQUE_POP_FRONT: super::ops::Pop
```



## handl_lib_collections::module::DEQUE_PUSH_BACK

*Static*

```rust
static DEQUE_PUSH_BACK: super::ops::Push
```



## handl_lib_collections::module::DEQUE_PUSH_FRONT

*Static*

```rust
static DEQUE_PUSH_FRONT: super::ops::Push
```



## handl_lib_collections::module::FROM

*Static*

```rust
static FROM: super::ops::From
```



## handl_lib_collections::module::HASH_MAP_FROM

*Static*

```rust
static HASH_MAP_FROM: hm::From
```



## handl_lib_collections::module::HASH_MAP_GET

*Static*

```rust
static HASH_MAP_GET: hm::Get
```



## handl_lib_collections::module::HASH_MAP_HAS

*Static*

```rust
static HASH_MAP_HAS: hm::Has
```



## handl_lib_collections::module::HASH_MAP_IS_EMPTY

*Static*

```rust
static HASH_MAP_IS_EMPTY: hm::IsEmpty
```



## handl_lib_collections::module::HASH_MAP_KEYS

*Static*

```rust
static HASH_MAP_KEYS: hm::Keys
```



## handl_lib_collections::module::HASH_MAP_LEN

*Static*

```rust
static HASH_MAP_LEN: hm::Len
```



## handl_lib_collections::module::HASH_MAP_NEW

*Static*

```rust
static HASH_MAP_NEW: hm::New
```



## handl_lib_collections::module::HASH_MAP_REMOVE

*Static*

```rust
static HASH_MAP_REMOVE: hm::Remove
```



## handl_lib_collections::module::HASH_MAP_SET

*Static*

```rust
static HASH_MAP_SET: hm::Set
```



## handl_lib_collections::module::HASH_MAP_TO_OBJECT

*Static*

```rust
static HASH_MAP_TO_OBJECT: hm::ToObject
```



## handl_lib_collections::module::HASH_MAP_VALUES

*Static*

```rust
static HASH_MAP_VALUES: hm::Values
```



## handl_lib_collections::module::HASH_SET_ADD

*Static*

```rust
static HASH_SET_ADD: hs::Add
```



## handl_lib_collections::module::HASH_SET_DIFFERENCE

*Static*

```rust
static HASH_SET_DIFFERENCE: hs::Difference
```



## handl_lib_collections::module::HASH_SET_FROM

*Static*

```rust
static HASH_SET_FROM: hs::From
```



## handl_lib_collections::module::HASH_SET_HAS

*Static*

```rust
static HASH_SET_HAS: hs::Has
```



## handl_lib_collections::module::HASH_SET_INTERSECTION

*Static*

```rust
static HASH_SET_INTERSECTION: hs::Intersection
```



## handl_lib_collections::module::HASH_SET_IS_EMPTY

*Static*

```rust
static HASH_SET_IS_EMPTY: hs::IsEmpty
```



## handl_lib_collections::module::HASH_SET_LEN

*Static*

```rust
static HASH_SET_LEN: hs::Len
```



## handl_lib_collections::module::HASH_SET_NEW

*Static*

```rust
static HASH_SET_NEW: hs::New
```



## handl_lib_collections::module::HASH_SET_REMOVE

*Static*

```rust
static HASH_SET_REMOVE: hs::Remove
```



## handl_lib_collections::module::HASH_SET_TO_ARRAY

*Static*

```rust
static HASH_SET_TO_ARRAY: hs::ToArray
```



## handl_lib_collections::module::HASH_SET_UNION

*Static*

```rust
static HASH_SET_UNION: hs::Union
```



## handl_lib_collections::module::IS_EMPTY

*Static*

```rust
static IS_EMPTY: super::ops::IsEmpty
```



## handl_lib_collections::module::LEN

*Static*

```rust
static LEN: super::ops::Len
```



## handl_lib_collections::module::LIST_PEEK

*Static*

```rust
static LIST_PEEK: super::ops::Peek
```



## handl_lib_collections::module::LIST_POP

*Static*

```rust
static LIST_POP: super::ops::Pop
```



## handl_lib_collections::module::LIST_PUSH

*Static*

```rust
static LIST_PUSH: super::ops::Push
```



## handl_lib_collections::module::NEW

*Static*

```rust
static NEW: super::ops::New
```



## handl_lib_collections::module::QUEUE_DEQUEUE

*Static*

```rust
static QUEUE_DEQUEUE: super::ops::Pop
```



## handl_lib_collections::module::QUEUE_ENQUEUE

*Static*

```rust
static QUEUE_ENQUEUE: super::ops::Push
```



## handl_lib_collections::module::QUEUE_PEEK

*Static*

```rust
static QUEUE_PEEK: super::ops::Peek
```



## handl_lib_collections::module::STACK_PEEK

*Static*

```rust
static STACK_PEEK: super::ops::Peek
```



## handl_lib_collections::module::STACK_POP

*Static*

```rust
static STACK_POP: super::ops::Pop
```



## handl_lib_collections::module::STACK_PUSH

*Static*

```rust
static STACK_PUSH: super::ops::Push
```



## handl_lib_collections::module::TO_ARRAY

*Static*

```rust
static TO_ARRAY: super::ops::ToArray
```



## handl_lib_collections::module::VEC_PEEK

*Static*

```rust
static VEC_PEEK: super::ops::Peek
```



## handl_lib_collections::module::VEC_POP

*Static*

```rust
static VEC_POP: super::ops::Pop
```



## handl_lib_collections::module::VEC_PUSH

*Static*

```rust
static VEC_PUSH: super::ops::Push
```



