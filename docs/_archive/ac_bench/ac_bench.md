**handl_bench**

# Module: handl_bench

## Contents

**Functions**

- [`arg_schemas`](#arg_schemas) - Generate `n` schema pairs where every field has an `arg:` key.
- [`array_schema`](#array_schema) - Generate a JSON schema for an array of the given item schema.
- [`array_type`](#array_type) - Generate an array type wrapping `inner`.
- [`expr_schemas`](#expr_schemas) - Generate `n` schema pairs where every field has an `expr:` key.
- [`expr_yaml`](#expr_yaml) - Generate a YAML document where every field has an `expr:` declaration.
- [`flat_yaml`](#flat_yaml) - Generate a flat YAML document with `n` simple typed fields.
- [`generic_call_exprs`](#generic_call_exprs) - Generate `n` expressions that call generic functions (array.map, array.filter, etc.).
- [`immutable_fields`](#immutable_fields) - Generate `n` immutable field names.
- [`inherit_yaml`](#inherit_yaml) - Generate a YAML document with `n` inheritable fields and `depth`
- [`inheritable_schemas`](#inheritable_schemas) - Generate `n` inheritable schema pairs.
- [`literal_expr`](#literal_expr) - Generate a literal expression of the given type index.
- [`merge_schemas`](#merge_schemas) - Generate `n` schema pairs where every field has `merge: true`.
- [`merge_yaml`](#merge_yaml) - Generate a YAML document with `n` merge fields.
- [`mixed_exprs`](#mixed_exprs) - Generate `n` distinct expression strings (mix of literals, paths, calls).
- [`mixed_yaml`](#mixed_yaml) - Generate a mixed-feature YAML document with `n` total fields,
- [`mutable_yaml`](#mutable_yaml) - Generate a YAML document with `n` immutable fields and `depth`
- [`nested_call_expr`](#nested_call_expr) - Generate a nested function call expression of `depth` levels.
- [`nested_data`](#nested_data) - Generate nested JSON data with `n` inheritable fields at each of `depth` levels.
- [`nested_data_uniform`](#nested_data_uniform) - Generate nested JSON data with `n` immutable fields at each of `depth` levels
- [`nested_object_schema`](#nested_object_schema) - Generate a JSON schema for a nested object with `depth` levels.
- [`nested_object_type`](#nested_object_type) - Generate a nested object type with `depth` levels.
- [`object_schema`](#object_schema) - Generate a JSON schema for an object with `n` properties.
- [`object_type`](#object_type) - Generate an object type with `n` properties.
- [`path_expr`](#path_expr) - Generate a path lookup expression of `depth` segments.
- [`simple_schema`](#simple_schema) - Generate a simple JSON schema fragment (one of the primitive types).
- [`simple_schemas`](#simple_schemas) - Generate `n` `(String, JsonValue)` schema pairs with simple types.
- [`simple_type`](#simple_type) - Generate a `Type` variant from an index.

---

## handl_bench::arg_schemas

*Function*

Generate `n` schema pairs where every field has an `arg:` key.

```rust
fn arg_schemas(n: usize) -> Vec<(String, serde_json::Value)>
```



## handl_bench::array_schema

*Function*

Generate a JSON schema for an array of the given item schema.

```rust
fn array_schema(item: serde_json::Value) -> serde_json::Value
```



## handl_bench::array_type

*Function*

Generate an array type wrapping `inner`.

```rust
fn array_type(inner: handl_typecheck_core::Type) -> handl_typecheck_core::Type
```



## handl_bench::expr_schemas

*Function*

Generate `n` schema pairs where every field has an `expr:` key.

```rust
fn expr_schemas(n: usize) -> Vec<(String, serde_json::Value)>
```



## handl_bench::expr_yaml

*Function*

Generate a YAML document where every field has an `expr:` declaration.

```rust
fn expr_yaml(n: usize) -> String
```



## handl_bench::flat_yaml

*Function*

Generate a flat YAML document with `n` simple typed fields.

```rust
fn flat_yaml(n: usize) -> String
```



## handl_bench::generic_call_exprs

*Function*

Generate `n` expressions that call generic functions (array.map, array.filter, etc.).
These exercise the TraitRegistry + unification hot path.

```rust
fn generic_call_exprs(n: usize) -> Vec<String>
```



## handl_bench::immutable_fields

*Function*

Generate `n` immutable field names.

```rust
fn immutable_fields(n: usize) -> Vec<String>
```



## handl_bench::inherit_yaml

*Function*

Generate a YAML document with `n` inheritable fields and `depth`
levels of nested overrides.

```rust
fn inherit_yaml(n: usize, depth: usize) -> String
```



## handl_bench::inheritable_schemas

*Function*

Generate `n` inheritable schema pairs.

```rust
fn inheritable_schemas(n: usize) -> Vec<(String, serde_json::Value)>
```



## handl_bench::literal_expr

*Function*

Generate a literal expression of the given type index.

```rust
fn literal_expr(i: usize) -> String
```



## handl_bench::merge_schemas

*Function*

Generate `n` schema pairs where every field has `merge: true`.

```rust
fn merge_schemas(n: usize) -> Vec<(String, serde_json::Value)>
```



## handl_bench::merge_yaml

*Function*

Generate a YAML document with `n` merge fields.

```rust
fn merge_yaml(n: usize) -> String
```



## handl_bench::mixed_exprs

*Function*

Generate `n` distinct expression strings (mix of literals, paths, calls).

```rust
fn mixed_exprs(n: usize) -> Vec<String>
```



## handl_bench::mixed_yaml

*Function*

Generate a mixed-feature YAML document with `n` total fields,
exercising expr, arg, inheritable, mutable, and merge.

```rust
fn mixed_yaml(n: usize) -> String
```



## handl_bench::mutable_yaml

*Function*

Generate a YAML document with `n` immutable fields and `depth`
levels of nested data (some overrides, some not).

```rust
fn mutable_yaml(n: usize, depth: usize) -> String
```



## handl_bench::nested_call_expr

*Function*

Generate a nested function call expression of `depth` levels.

```rust
fn nested_call_expr(depth: usize) -> String
```



## handl_bench::nested_data

*Function*

Generate nested JSON data with `n` inheritable fields at each of `depth` levels.

```rust
fn nested_data(n: usize, depth: usize) -> serde_json::Value
```



## handl_bench::nested_data_uniform

*Function*

Generate nested JSON data with `n` immutable fields at each of `depth` levels
(same values as root — no violations, just traversal cost).

```rust
fn nested_data_uniform(n: usize, depth: usize) -> serde_json::Value
```



## handl_bench::nested_object_schema

*Function*

Generate a JSON schema for a nested object with `depth` levels.

```rust
fn nested_object_schema(depth: usize) -> serde_json::Value
```



## handl_bench::nested_object_type

*Function*

Generate a nested object type with `depth` levels.

```rust
fn nested_object_type(depth: usize) -> handl_typecheck_core::Type
```



## handl_bench::object_schema

*Function*

Generate a JSON schema for an object with `n` properties.

```rust
fn object_schema(n: usize) -> serde_json::Value
```



## handl_bench::object_type

*Function*

Generate an object type with `n` properties.

```rust
fn object_type(n: usize) -> handl_typecheck_core::Type
```



## handl_bench::path_expr

*Function*

Generate a path lookup expression of `depth` segments.

```rust
fn path_expr(depth: usize) -> String
```



## handl_bench::simple_schema

*Function*

Generate a simple JSON schema fragment (one of the primitive types).

```rust
fn simple_schema(i: usize) -> serde_json::Value
```



## handl_bench::simple_schemas

*Function*

Generate `n` `(String, JsonValue)` schema pairs with simple types.

```rust
fn simple_schemas(n: usize) -> Vec<(String, serde_json::Value)>
```



## handl_bench::simple_type

*Function*

Generate a `Type` variant from an index.

```rust
fn simple_type(i: usize) -> handl_typecheck_core::Type
```



