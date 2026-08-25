**handl_fields > interpolation**

# Module: interpolation

## Contents

**Functions**

- [`extract_single_expr`](#extract_single_expr) - Checks if `s` is exactly a single interpolation expression (nothing before or after).
- [`interpolate`](#interpolate) - Recursively interpolates expressions in string values.
- [`interpolate_scoped`](#interpolate_scoped) - Recursively interpolates expressions in string values with scope inheritance.
- [`interpolate_with_scope`](#interpolate_with_scope) - Internal: scope-aware interpolation. Carries a `scope` (inheritable field
- [`resolve_interpolation`](#resolve_interpolation) - Replaces all interpolation patterns in a string, returning a `JsonValue`.

---

## handl_fields::interpolation::extract_single_expr

*Function*

Checks if `s` is exactly a single interpolation expression (nothing before or after).
Returns the inner expression text if so.

```rust
fn extract_single_expr<'a>(s: &'a str, prefix: &str, suffix: &str) -> Option<&'a str>
```



## handl_fields::interpolation::interpolate

*Function*

Recursively interpolates expressions in string values.

Supports (using the canonical handl dialect):
- Namespace paths: `${doc.name}`, `${env.HOME}`, `${doc.author.email}`
- Shorthand paths: `${name}` (equivalent to `${doc.name}`)
- Function calls: `${now()}`, `${add(1, 2)}`, `${concat(doc.name, "!")}`
- Nested calls: `${add(mul(2, 3), 4)}`

Function calls are resolved via `host`. If no functions are needed,
use `ac_runtime_example_1::DefaultHost`.

```rust
fn interpolate<H>(value: &serde_json::Value, namespaces: &crate::types::Namespaces, host: &H, dialect: &'static dyn Dialect) -> serde_json::Value
```



## handl_fields::interpolation::interpolate_scoped

*Function*

Recursively interpolates expressions in string values with scope inheritance.

Like [`interpolate`], but as the recursion descends into nested objects,
fields declared as `inheritable: true` are merged into the default namespace.
Child objects inherit the parent's scope and can override inheritable fields
locally — the override applies to that subtree only.

If `inheritable` is empty, this is equivalent to [`interpolate`].

```rust
fn interpolate_scoped<H>(value: &serde_json::Value, namespaces: &crate::types::Namespaces, inheritable: &[String], host: &H, dialect: &'static dyn Dialect) -> serde_json::Value
```



## handl_fields::interpolation::interpolate_with_scope

*Function*

Internal: scope-aware interpolation. Carries a `scope` (inheritable field
values from parent objects) and merges it into the default namespace at each
object level.

```rust
fn interpolate_with_scope<H>(value: &serde_json::Value, namespaces: &crate::types::Namespaces, inheritable: &std::collections::HashSet<String>, scope: &serde_json::Map<String, serde_json::Value>, host: &H, dialect: &'static dyn Dialect) -> serde_json::Value
```



## handl_fields::interpolation::resolve_interpolation

*Function*

Replaces all interpolation patterns in a string, returning a `JsonValue`.
If the string is a single expression, the raw value is returned (preserving type).
Otherwise, all patterns are stringified and concatenated.

```rust
fn resolve_interpolation<H>(s: &str, namespaces: &crate::types::Namespaces, host: &H, dialect: &'static dyn Dialect) -> serde_json::Value
```



