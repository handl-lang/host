**handl_lib_utils > array > higher_order**

# Module: array::higher_order

## Contents

**Structs**

- [`Chunk`](#chunk)
- [`Compact`](#compact)
- [`CountBy`](#countby)
- [`Every`](#every)
- [`Filter`](#filter)
- [`Find`](#find)
- [`FlatMap`](#flatmap)
- [`GroupBy`](#groupby)
- [`Map`](#map)
- [`MaxBy`](#maxby)
- [`MinBy`](#minby)
- [`Partition`](#partition)
- [`Pluck`](#pluck)
- [`Range`](#range)
- [`Reduce`](#reduce)
- [`SomeFn`](#somefn)
- [`SortBy`](#sortby)
- [`SortByDesc`](#sortbydesc)
- [`Zip`](#zip)

**Functions**

- [`extremum_by_impl`](#extremum_by_impl)
- [`sort_by_impl`](#sort_by_impl)

---

## handl_lib_utils::array::higher_order::Chunk

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Chunk`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::array::higher_order::Compact

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **GenericFunction**
  - `fn call_with_types(self: &Self, args: &[NonFunctionValue], _type_args: &[String], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Compact`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::array::higher_order::CountBy

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> CountBy`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **GenericFunction**
  - `fn call_with_types(self: &Self, args: &[NonFunctionValue], _type_args: &[String], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::array::higher_order::Every

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Every`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **GenericFunction**
  - `fn call_with_types(self: &Self, args: &[NonFunctionValue], _type_args: &[String], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::array::higher_order::Filter

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **GenericFunction**
  - `fn call_with_types(self: &Self, args: &[NonFunctionValue], _type_args: &[String], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Filter`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::array::higher_order::Find

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Find`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **GenericFunction**
  - `fn call_with_types(self: &Self, args: &[NonFunctionValue], _type_args: &[String], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::array::higher_order::FlatMap

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **GenericFunction**
  - `fn call_with_types(self: &Self, args: &[NonFunctionValue], _type_args: &[String], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> FlatMap`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::array::higher_order::GroupBy

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **GenericFunction**
  - `fn call_with_types(self: &Self, args: &[NonFunctionValue], _type_args: &[String], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> GroupBy`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::array::higher_order::Map

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Map`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **GenericFunction**
  - `fn call_with_types(self: &Self, args: &[NonFunctionValue], _type_args: &[String], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::array::higher_order::MaxBy

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **GenericFunction**
  - `fn call_with_types(self: &Self, args: &[NonFunctionValue], _type_args: &[String], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> MaxBy`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::array::higher_order::MinBy

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> MinBy`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **GenericFunction**
  - `fn call_with_types(self: &Self, args: &[NonFunctionValue], _type_args: &[String], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::array::higher_order::Partition

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Partition`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **GenericFunction**
  - `fn call_with_types(self: &Self, args: &[NonFunctionValue], _type_args: &[String], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::array::higher_order::Pluck

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Pluck`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **GenericFunction**
  - `fn call_with_types(self: &Self, args: &[NonFunctionValue], _type_args: &[String], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::array::higher_order::Range

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Range`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::array::higher_order::Reduce

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Reduce`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **GenericFunction**
  - `fn call_with_types(self: &Self, args: &[NonFunctionValue], _type_args: &[String], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::array::higher_order::SomeFn

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **GenericFunction**
  - `fn call_with_types(self: &Self, args: &[NonFunctionValue], _type_args: &[String], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> SomeFn`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::array::higher_order::SortBy

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **GenericFunction**
  - `fn call_with_types(self: &Self, args: &[NonFunctionValue], _type_args: &[String], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> SortBy`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::array::higher_order::SortByDesc

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> SortByDesc`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **GenericFunction**
  - `fn call_with_types(self: &Self, args: &[NonFunctionValue], _type_args: &[String], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::array::higher_order::Zip

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> Zip`



## handl_lib_utils::array::higher_order::extremum_by_impl

*Function*

```rust
fn extremum_by_impl(args: &[ac_runtime_values::NonFunctionValue], ctx: & mut dyn Context, pick_max: bool) -> Result<ac_runtime_values::NonFunctionValue, ac_runtime_values::Error>
```



## handl_lib_utils::array::higher_order::sort_by_impl

*Function*

```rust
fn sort_by_impl(args: &[ac_runtime_values::NonFunctionValue], ctx: & mut dyn Context, descending: bool) -> Result<ac_runtime_values::NonFunctionValue, ac_runtime_values::Error>
```



