**handl_typecheck_core > trait_registry**

# Module: trait_registry

## Contents

**Structs**

- [`TraitRegistry`](#traitregistry) - A registry mapping trait names to the concrete types that implement them.

---

## handl_typecheck_core::trait_registry::TraitRegistry

*Struct*

A registry mapping trait names to the concrete types that implement them.

Used during generic call instantiation to verify that a type variable's
bounds are satisfied by the concrete type it binds to.

# Built-in traits

The [`TraitRegistry::default`] registry includes:
- `Numeric` — `Number`
- `Textual` — `String`
- `Boolean` — `Boolean`
- `Nullable` — `Null`
- `Comparable` — `String`, `Number`, `Boolean`
- `Serializable` — all types (everything in handl is JSON-serializable)

Hosts can register custom traits via [`TraitRegistry::register`].

**Fields:**
- `traits: std::collections::BTreeMap<String, Vec<crate::Type>>`

**Methods:**

- `fn new() -> Self` - Create an empty registry.
- `fn default_registry() -> Self` - Create a registry with built-in traits.
- `fn default_registry_ref() -> &'static TraitRegistry` - Return a reference to the static default registry.
- `fn register<impl Into<String>>(self: & mut Self, trait_name: impl Trait, ty: Type)` - Register a type as implementing a trait.
- `fn implements(self: &Self, trait_name: &str, ty: &Type) -> bool` - Check if a type implements a named trait.
- `fn check_bounds(self: &Self, ty: &Type, bounds: &[Type]) -> bool` - Check if a concrete type satisfies all given bounds.

**Traits:** Eq

**Trait Implementations:**

- **Default**
  - `fn default() -> Self`
- **PartialEq**
  - `fn eq(self: &Self, other: &TraitRegistry) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> TraitRegistry`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



