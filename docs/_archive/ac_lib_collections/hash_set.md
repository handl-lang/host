**handl_lib_collections > hash_set**

# Module: hash_set

## Contents

**Structs**

- [`Add`](#add) - `collections.hash_set.add(set: set, value: value) → set`
- [`Difference`](#difference) - `collections.hash_set.difference(a: set, b: set) → set`
- [`From`](#from) - `collections.hash_set.from(array: array) → set`
- [`Has`](#has) - `collections.hash_set.has(set: set, value: value) → bool`
- [`Intersection`](#intersection) - `collections.hash_set.intersection(a: set, b: set) → set`
- [`IsEmpty`](#isempty) - `collections.hash_set.is_empty(set: set) → bool`
- [`Len`](#len) - `collections.hash_set.len(set: set) → number`
- [`New`](#new) - `collections.hash_set.new() → set`
- [`Remove`](#remove) - `collections.hash_set.remove(set: set, value: value) → set`
- [`ToArray`](#toarray) - `collections.hash_set.to_array(set: set) → array`
- [`Union`](#union) - `collections.hash_set.union(a: set, b: set) → set`

**Functions**

- [`has_value`](#has_value)

---

## handl_lib_collections::hash_set::Add

*Struct*

`collections.hash_set.add(set: set, value: value) → set`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Add`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_collections::hash_set::Difference

*Struct*

`collections.hash_set.difference(a: set, b: set) → set`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Difference`



## handl_lib_collections::hash_set::From

*Struct*

`collections.hash_set.from(array: array) → set`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> From`



## handl_lib_collections::hash_set::Has

*Struct*

`collections.hash_set.has(set: set, value: value) → bool`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Has`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_collections::hash_set::Intersection

*Struct*

`collections.hash_set.intersection(a: set, b: set) → set`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Intersection`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_collections::hash_set::IsEmpty

*Struct*

`collections.hash_set.is_empty(set: set) → bool`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> IsEmpty`



## handl_lib_collections::hash_set::Len

*Struct*

`collections.hash_set.len(set: set) → number`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Len`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_collections::hash_set::New

*Struct*

`collections.hash_set.new() → set`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> New`
- **Function**
  - `fn call(self: &Self, _args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_collections::hash_set::Remove

*Struct*

`collections.hash_set.remove(set: set, value: value) → set`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Remove`



## handl_lib_collections::hash_set::ToArray

*Struct*

`collections.hash_set.to_array(set: set) → array`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> ToArray`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_collections::hash_set::Union

*Struct*

`collections.hash_set.union(a: set, b: set) → set`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Union`



## handl_lib_collections::hash_set::has_value

*Function*

```rust
fn has_value(input: &[serde_json::Value], target: &serde_json::Value) -> bool
```



