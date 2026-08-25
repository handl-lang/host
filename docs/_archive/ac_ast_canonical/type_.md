**handl_ast_canonical > type_**

# Module: type_

## Contents

**Structs**

- [`CanonicalTypeFactory`](#canonicaltypefactory) - The canonical handl type factory.

**Enums**

- [`Type`](#type) - A type in the handl language.

---

## handl_ast_canonical::type_::CanonicalTypeFactory

*Struct*

The canonical handl type factory.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **TypeFactory**
  - `fn primitive<S>(self: &Self, name: S) -> Type`
  - `fn array(self: &Self, element: Type) -> Type`
  - `fn object(self: &Self, fields: Vec<(String, Type)>) -> Type`
  - `fn function(self: &Self, params: Vec<Type>, ret: Type) -> Type`
- **Clone**
  - `fn clone(self: &Self) -> CanonicalTypeFactory`
- **Default**
  - `fn default() -> CanonicalTypeFactory`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_ast_canonical::type_::Type

*Enum*

A type in the handl language.

The canonical type AST supports primitives, arrays, objects, and
function types. More advanced forms (generics, type variables, unions)
can be added behind new variants as the type system grows.

**Variants:**
- `Primitive(String)` - A named primitive type such as `string`, `number`, or `boolean`.
- `Array(Box<Type>)` - An array whose elements are all of the contained type.
- `Object(Vec<(String, Type)>)` - An object with a fixed set of named fields.
- `Function{ params: Vec<Type>, ret: Box<Type> }` - A function from parameter types to a return type.

**Methods:**

- `fn primitive<impl Into<String>>(name: impl Trait) -> Self` - Creates a primitive type.
- `fn array(element: Type) -> Self` - Creates an array type.
- `fn object(fields: Vec<(String, Type)>) -> Self` - Creates an object type.
- `fn function(params: Vec<Type>, ret: Type) -> Self` - Creates a function type.

**Traits:** Eq

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &Type) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> Type`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



