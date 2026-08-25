**handl_typegen_import**

# Module: handl_typegen_import

## Contents

**Modules**

- [`import_detect`](#import_detect)
- [`import_resolve`](#import_resolve)

**Structs**

- [`DefaultImportResolver`](#defaultimportresolver) - Default import resolver for the handl typegen system.

**Functions**

- [`apply_imports`](#apply_imports) - Apply import resolution to a `TypegenContext`.

---

## handl_typegen_import::DefaultImportResolver

*Struct*

Default import resolver for the handl typegen system.

Implements [`ImportResolver`] by following `import()` expressions
in merge fields, resolving paths via a [`ModuleResolver`], and
merging imported schemas into the parent context.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> DefaultImportResolver`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> DefaultImportResolver`
- **ImportResolver**
  - `fn resolve_imports(self: &Self, ctx: & mut TypegenContext, schemas: &[(String, JsonValue)], merge_fields: &[String], data: &JsonValue, resolver: &dyn ModuleResolver, base: Option<&Path>, vocab: &dyn DocumentVocabulary)`



## handl_typegen_import::apply_imports

*Function*

Apply import resolution to a `TypegenContext`.

For each merge field with an `import()` expression, follows the import
path via `resolver`, extracts the imported module's fields, filters
merge fields, strips handl keys, and merges them into the context
(parent wins).

`vocab` provides the reserved key names used for stripping handl-specific
keys from imported module schemas.

```rust
fn apply_imports(ctx: & mut handl_typegen_core::TypegenContext, schemas: &[(String, serde_json::Value)], merge_fields: &[String], data: &serde_json::Value, resolver: &dyn ModuleResolver, base: Option<&std::path::Path>, vocab: &dyn DocumentVocabulary)
```



## Module: import_detect



## Module: import_resolve



