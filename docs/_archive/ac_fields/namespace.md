**handl_fields > namespace**

# Module: namespace

## Contents

**Functions**

- [`build_doc_namespace`](#build_doc_namespace) - Builds the default namespace from the document's data fields.
- [`build_env_namespace`](#build_env_namespace) - Builds an `env` namespace from process environment variables.

---

## handl_fields::namespace::build_doc_namespace

*Function*

Builds the default namespace from the document's data fields.

```rust
fn build_doc_namespace(data: &ac_runtime_values::NonFunctionValue) -> serde_json::Value
```



## handl_fields::namespace::build_env_namespace

*Function*

Builds an `env` namespace from process environment variables.

```rust
fn build_env_namespace() -> serde_json::Value
```



