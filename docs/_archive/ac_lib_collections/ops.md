**handl_lib_collections > ops**

# Module: ops

## Contents

**Structs**

- [`From`](#from) - `collections.*.from(array: array) → collection`
- [`IsEmpty`](#isempty) - `collections.*.is_empty(collection: collection) → bool`
- [`Len`](#len) - `collections.*.len(collection: collection) → number`
- [`New`](#new) - `collections.*.new() → collection`
- [`Peek`](#peek) - `collections.*.peek(collection: collection) → value`
- [`Pop`](#pop) - `collections.*.pop(collection: collection) → collection`
- [`Push`](#push) - `collections.*.push(collection: collection, value: value) → collection`
- [`ToArray`](#toarray) - `collections.*.to_array(collection: collection) → array`

**Enums**

- [`End`](#end) - Which end of an array-backed collection to operate on.

---

## handl_lib_collections::ops::End

*Enum*

Which end of an array-backed collection to operate on.

**Variants:**
- `Front`
- `Back`

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> End`



## handl_lib_collections::ops::From

*Struct*

`collections.*.from(array: array) → collection`

Creates a collection from an existing array.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> From`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_collections::ops::IsEmpty

*Struct*

`collections.*.is_empty(collection: collection) → bool`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> IsEmpty`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_collections::ops::Len

*Struct*

`collections.*.len(collection: collection) → number`

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Len`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_collections::ops::New

*Struct*

`collections.*.new() → collection`

Returns a new empty collection.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> New`
- **Function**
  - `fn call(self: &Self, _args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_collections::ops::Peek

*Struct*

`collections.*.peek(collection: collection) → value`

Returns the value at the chosen end without removing it.

**Fields:**
- `end: End`

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Peek`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_collections::ops::Pop

*Struct*

`collections.*.pop(collection: collection) → collection`

Removes the value at the chosen end and returns the shorter collection.

**Fields:**
- `end: End`

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Pop`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_collections::ops::Push

*Struct*

`collections.*.push(collection: collection, value: value) → collection`

Pushes a value onto the chosen end of the collection.

**Fields:**
- `end: End`

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Push`



## handl_lib_collections::ops::ToArray

*Struct*

`collections.*.to_array(collection: collection) → array`

Returns the collection as a plain array.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> ToArray`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



