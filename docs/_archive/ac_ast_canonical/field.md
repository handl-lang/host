**handl_ast_canonical > field**

# Module: field

## Contents

**Structs**

- [`Field`](#field) - A named binding of a value.

---

## handl_ast_canonical::field::Field

*Struct*

A named binding of a value.

A field associates a name with a value. Fields are the building
blocks of objects and configurations — an object is conceptually
a collection of fields.

`Field` is generic over the value type `V`, keeping `handl-ast`
dependency-free.

**Generic Parameters:**
- V

**Fields:**
- `name: String` - The field name.
- `value: V` - The field's value.

**Methods:**

- `fn new<impl Into<String>>(name: impl Trait, value: V) -> Self` - Creates a new field.

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &Field<V>) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> Field<V>`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



