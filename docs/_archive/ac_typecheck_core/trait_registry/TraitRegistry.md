**handl_typecheck_core > trait_registry > TraitRegistry**

# Module: trait_registry::TraitRegistry

## Contents

**Functions**

- [`default`](#default)
- [`register`](#register) - Register a type as implementing a trait.

---

## handl_typecheck_core::trait_registry::TraitRegistry::default

*Function*

```rust
fn default() -> Self
```



## handl_typecheck_core::trait_registry::TraitRegistry::register

*Function*

Register a type as implementing a trait.

```rust
fn register<impl Into<String>>(self: & mut Self, trait_name: impl Trait, ty: Type)
```



