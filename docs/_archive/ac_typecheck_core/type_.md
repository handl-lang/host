**handl_typecheck_core > type_**

# Module: type_

## Contents

**Structs**

- [`FunctionType`](#functiontype) - A function type — type parameters, parameter names/types, and return type.
- [`TypeParam`](#typeparam) - A type parameter declaration on a generic function.

**Enums**

- [`Kind`](#kind) - The kind of a type — tracks the "type of types" for higher-kinded types.
- [`Type`](#type) - A static type in the handl type system.
- [`Variance`](#variance) - Variance annotations for type parameters.

---

## handl_typecheck_core::type_::FunctionType

*Struct*

A function type — type parameters, parameter names/types, and return type.

`type_params` is empty for monomorphic functions. When non-empty, the
function is generic and the typechecker unifies argument types against
parameter types to instantiate the return type.

**Fields:**
- `type_params: Vec<TypeParam>` - Type parameters (e.g. `[TypeParam::new("T")]`). Empty for monomorphic functions.
- `params: Vec<(String, Type)>` - `(param_name, param_type)` pairs, in declaration order.
- `return_type: Box<Type>` - The return type.

**Traits:** Eq

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &FunctionType) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> FunctionType`



## handl_typecheck_core::type_::Kind

*Enum*

The kind of a type — tracks the "type of types" for higher-kinded types.

- [`Kind::Type`]: a concrete type (e.g. `String`, `Number`, `Array<String>`).
- [`Kind::Constructor`]: a type constructor that takes arguments.
  The `Vec<Kind>` describes the argument kinds, and the boxed `Kind`
  describes the result kind.

Examples:
- `String` has kind `Type`
- `Array` has kind `Type -> Type` (takes one type arg, produces a type)
- `Map` has kind `Type -> Type -> Type`
- `F` (a higher-kinded type variable) has kind `Type -> Type`

**Variants:**
- `Type`
- `Constructor(Vec<Kind>, Box<Kind>)`

**Methods:**

- `fn type_kind() -> Self` - The kind of a proper type (no arguments).
- `fn unary() -> Self` - Construct a `Type -> Type` kind (unary type constructor).
- `fn binary() -> Self` - Construct a `Type -> Type -> Type` kind (binary type constructor).

**Traits:** Eq

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Kind`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &Kind) -> bool`



## handl_typecheck_core::type_::Type

*Enum*

A static type in the handl type system.

Types are derived from schema fragments via a [`SchemaInterpreter`].
The typechecker compares inferred types against declared types to
detect mismatches before runtime evaluation.

`Unknown` is compatible with everything — it represents types that
cannot be statically determined. `Never` represents impossible code
paths (e.g., a function that always errors).

`Var` represents a type variable used in generic function signatures.
Outside of unification (during generic call instantiation), `Var` is
not compatible with concrete types. See [`crate::unify`].

`Trait` represents a named trait reference used in type parameter bounds.
A `Trait` is never a concrete type — it only appears in `TypeParam::bounds`
and is resolved against a [`crate::TraitRegistry`] during bound checking.

**Variants:**
- `String`
- `Number`
- `Boolean`
- `Null`
- `Array(Box<Type>)`
- `Object(std::collections::BTreeMap<String, Type>)`
- `Function(FunctionType)`
- `Var(String)` - A type variable (e.g. `T`, `U`) used in generic function signatures.
- `Trait(String)` - A named trait reference (e.g. `Numeric`, `Comparable`) used in type
- `App(Box<Type>, Vec<Type>)` - A type application: applying a type constructor to type arguments.
- `Unknown`
- `Never`

**Methods:**

- `fn name(self: &Self) -> String` - Human-readable name for error messages.
- `fn is_unknown(self: &Self) -> bool` - Returns `true` if this type is `Unknown`.
- `fn to_json(self: &Self) -> JsonValue` - Encode this type as a JSON value.
- `fn from_json(value: &JsonValue) -> Option<Self>` - Decode a type from a JSON value produced by [`to_json`].

**Traits:** Eq

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &Type) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> Type`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_typecheck_core::type_::TypeParam

*Struct*

A type parameter declaration on a generic function.

Each type parameter has a name (e.g. `"T"`), an optional list of trait
bounds (e.g. `[Trait("Numeric")]`), and a variance annotation.

Variance defaults to [`Variance::Invariant`] — the safest default.
See [`Variance`] for details.

**Fields:**
- `name: String` - The type parameter name (e.g. `"T"`, `"U"`).
- `bounds: Vec<Type>` - Trait bounds that the concrete type must satisfy (e.g. `[Trait("Numeric")]`).
- `variance: Variance` - Variance annotation for subtyping. Defaults to [`Variance::Invariant`].

**Methods:**

- `fn new<impl Into<String>>(name: impl Trait) -> Self` - Create a type parameter with no bounds and invariant variance.
- `fn with_bounds<impl Into<String>>(name: impl Trait, bounds: Vec<Type>) -> Self` - Create a type parameter with the given trait bounds.
- `fn with_variance(self: Self, variance: Variance) -> Self` - Set the variance of this type parameter.

**Traits:** Eq

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &TypeParam) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> TypeParam`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_typecheck_core::type_::Variance

*Enum*

Variance annotations for type parameters.

Controls how subtyping flows through a type parameter's positions:
- **Covariant** (`out`): `Array<out T>` — `Array<String>` is a subtype of
  `Array<Unknown>` if `String` is a subtype of `Unknown`.
- **Contravariant** (`in`): `fn(in T) -> U` — a function accepting
  `Unknown` is usable where one accepting `String` is expected.
- **Invariant** (default): `Vec<T>` — no subtyping. The concrete type must
  match exactly.
- **Bivariant**: compatible in both directions — rarely useful, included
  for completeness.

**Variants:**
- `Covariant`
- `Contravariant`
- `Invariant`
- `Bivariant`

**Traits:** Eq, Copy

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &Variance) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> Variance`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



