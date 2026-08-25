**handl_typecheck_schema_json > interpret**

# Module: interpret

## Contents

**Functions**

- [`infer_json_type`](#infer_json_type) - Infer the [`Type`] of a concrete JSON value.
- [`interpret_function`](#interpret_function) - Build a [`FunctionType`] from a raw function signature.
- [`interpret_schema`](#interpret_schema) - Convert a JSON Schema fragment into a [`Type`].

---

## handl_typecheck_schema_json::interpret::infer_json_type

*Function*

Infer the [`Type`] of a concrete JSON value.

```rust
fn infer_json_type(value: &serde_json::Value) -> handl_typecheck_core::Type
```



## handl_typecheck_schema_json::interpret::interpret_function

*Function*

Build a [`FunctionType`] from a raw function signature.

`params` is `(param_name, raw_schema)` pairs.
`return_schema` is the raw return type schema, if declared.

```rust
fn interpret_function(params: &[(String, serde_json::Value)], return_schema: Option<&serde_json::Value>) -> handl_typecheck_core::FunctionType
```



## handl_typecheck_schema_json::interpret::interpret_schema

*Function*

Convert a JSON Schema fragment into a [`Type`].

In addition to standard JSON Schema `type` keywords, this interpreter
recognizes an handl-specific extension: `{ "type": "var", "name": "T" }`
produces `Type::Var("T")`, used in generic function signatures.

```rust
fn interpret_schema(schema: &serde_json::Value) -> handl_typecheck_core::Type
```



