**handl_lib_test > property**

# Module: property

## Contents

**Structs**

- [`ForAll`](#forall)
- [`PropertyModule`](#propertymodule) - The `property` module — property-based testing with seeded RNG.

---

## handl_lib_test::property::ForAll

*Struct*

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> ForAll`
- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error>`



## handl_lib_test::property::PropertyModule

*Struct*

The `property` module — property-based testing with seeded RNG.

Functions: `property.for_all`.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
- **Clone**
  - `fn clone(self: &Self) -> PropertyModule`



