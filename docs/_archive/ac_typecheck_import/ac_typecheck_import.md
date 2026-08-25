**handl_typecheck_import**

# Module: handl_typecheck_import

## Contents

**Modules**

- [`detect`](#detect)

**Functions**

- [`check_imports`](#check_imports) - Check type compatibility across imported modules.
- [`check_imports_inner`](#check_imports_inner)

---

## handl_typecheck_import::check_imports

*Function*

Check type compatibility across imported modules.

For each merge field with an `import()` expression:
1. Resolves the module via `resolver`.
2. Builds a [`TypeEnv`] from the imported module's schemas.
3. Checks that imported field types are compatible with the parent
   scope's declared types for the same field names.

Recursively checks nested imports from imported modules.

Returns a list of [`TypeError`]s for any incompatibilities.

```rust
fn check_imports(schemas: &[(String, serde_json::Value)], merge_fields: &[String], data: &serde_json::Value, resolver: &dyn ModuleResolver, interpreter: &dyn SchemaInterpreter, parent_env: &handl_typecheck_core::TypeEnv, base: Option<&std::path::Path>) -> Vec<handl_typecheck_core::TypeError>
```



## handl_typecheck_import::check_imports_inner

*Function*

```rust
fn check_imports_inner(schemas: &[(String, serde_json::Value)], merge_fields: &[String], data: &serde_json::Value, resolver: &dyn ModuleResolver, interpreter: &dyn SchemaInterpreter, parent_env: &handl_typecheck_core::TypeEnv, base: Option<&std::path::Path>, visited: & mut std::collections::HashSet<String>) -> Vec<handl_typecheck_core::TypeError>
```



## Module: detect



