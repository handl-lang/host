**handl_lib_collections > hash_map**

# Module: hash_map

## Contents

**Structs**

- [`From`](#from) - `collections.hash_map.from(object: object) → object`
- [`Get`](#get) - `collections.hash_map.get(map: hash_map, key: string, default?: value) → value`
- [`Has`](#has) - `collections.hash_map.has(map: hash_map, key: string) → bool`
- [`IsEmpty`](#isempty) - `collections.hash_map.is_empty(map: hash_map) → bool`
- [`Keys`](#keys) - `collections.hash_map.keys(map: hash_map) → array<string>`
- [`Len`](#len) - `collections.hash_map.len(map: hash_map) → number`
- [`New`](#new) - `collections.hash_map.new() → object`
- [`Remove`](#remove) - `collections.hash_map.remove(map: hash_map, key: string) → hash_map`
- [`Set`](#set) - `collections.hash_map.set(map: hash_map, key: string, value: value) → hash_map`
- [`ToObject`](#toobject) - `collections.hash_map.to_object(map: hash_map) → object`
- [`Values`](#values) - `collections.hash_map.values(map: hash_map) → array`

**Functions**

- [`require_key`](#require_key)

---

## handl_lib_collections::hash_map::From

*Struct*

`collections.hash_map.from(object: object) → object`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> From`



## handl_lib_collections::hash_map::Get

*Struct*

`collections.hash_map.get(map: hash_map, key: string, default?: value) → value`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Get`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_collections::hash_map::Has

*Struct*

`collections.hash_map.has(map: hash_map, key: string) → bool`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Has`



## handl_lib_collections::hash_map::IsEmpty

*Struct*

`collections.hash_map.is_empty(map: hash_map) → bool`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> IsEmpty`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_collections::hash_map::Keys

*Struct*

`collections.hash_map.keys(map: hash_map) → array<string>`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Keys`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_collections::hash_map::Len

*Struct*

`collections.hash_map.len(map: hash_map) → number`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Len`



## handl_lib_collections::hash_map::New

*Struct*

`collections.hash_map.new() → object`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, _args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> New`



## handl_lib_collections::hash_map::Remove

*Struct*

`collections.hash_map.remove(map: hash_map, key: string) → hash_map`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Remove`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_collections::hash_map::Set

*Struct*

`collections.hash_map.set(map: hash_map, key: string, value: value) → hash_map`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Set`



## handl_lib_collections::hash_map::ToObject

*Struct*

`collections.hash_map.to_object(map: hash_map) → object`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> ToObject`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_collections::hash_map::Values

*Struct*

`collections.hash_map.values(map: hash_map) → array`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Values`



## handl_lib_collections::hash_map::require_key

*Function*

```rust
fn require_key(args: &[ac_runtime_values::NonFunctionValue], idx: usize) -> Result<&str, ac_runtime_values::Error>
```



