**handl_typegen_import > import_resolve**

# Module: import_resolve

## Contents

**Functions**

- [`resolve_imports`](#resolve_imports) - Resolve imports from merge fields.

---

## handl_typegen_import::import_resolve::resolve_imports

*Function*

Resolve imports from merge fields.

For each merge field, checks if its `expr` contains an `import()` call.
If so, resolves the module via `resolver`, filters merge fields,
strips handl keys, and returns the imported field schemas.

Recursively resolves nested imports from imported modules.

Returns a list of (path, imported_field_schemas) pairs.

```rust
fn resolve_imports(schemas: &[(String, serde_json::Value)], merge_fields: &[String], data: &serde_json::Value, resolver: &dyn ModuleResolver, base: Option<&std::path::Path>, vocab: &dyn DocumentVocabulary) -> Vec<(String, Vec<(String, serde_json::Value)>)>
```



