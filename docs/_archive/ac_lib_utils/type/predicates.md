**handl_lib_utils > type > predicates**

# Module: type::predicates

## Contents

**Macros**

- [`type_predicate`](#type_predicate)

**Structs**

- [`IsArray`](#isarray)
- [`IsBool`](#isbool)
- [`IsEmpty`](#isempty)
- [`IsNull`](#isnull)
- [`IsNumber`](#isnumber)
- [`IsObject`](#isobject)
- [`IsString`](#isstring)
- [`IsTruthy`](#istruthy)
- [`TypeOf`](#typeof)

**Functions**

- [`b`](#b)
- [`require_one_arg`](#require_one_arg)
- [`s`](#s)

---

## handl_lib_utils::type::predicates::IsArray

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> IsArray`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::type::predicates::IsBool

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> IsBool`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::type::predicates::IsEmpty

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> IsEmpty`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::type::predicates::IsNull

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> IsNull`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::type::predicates::IsNumber

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`
- **Clone**
  - `fn clone(self: &Self) -> IsNumber`



## handl_lib_utils::type::predicates::IsObject

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> IsObject`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::type::predicates::IsString

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> IsString`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::type::predicates::IsTruthy

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> IsTruthy`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::type::predicates::TypeOf

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> TypeOf`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_utils::type::predicates::b

*Function*

```rust
fn b(value: bool) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_utils::type::predicates::require_one_arg

*Function*

```rust
fn require_one_arg(args: &[ac_runtime_values::NonFunctionValue]) -> Result<&ac_runtime_values::NonFunctionValue, ac_runtime_values::Error>
```



## handl_lib_utils::type::predicates::s

*Function*

```rust
fn s(value: &str) -> ac_runtime_values::NonFunctionValue
```



## handl_lib_utils::type::predicates::type_predicate

*Declarative Macro*

```rust
macro_rules! type_predicate {
    ($struct_name:ident, $predicate:expr) => { ... };
}
```



