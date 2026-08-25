**handl_frontend_yaml > convert**

# Module: convert

## Contents

**Functions**

- [`json_to_yaml`](#json_to_yaml)
- [`yaml_to_json`](#yaml_to_json)

---

## handl_frontend_yaml::convert::json_to_yaml

*Function*

```rust
fn json_to_yaml(value: &serde_json::Value) -> serde_yaml::Value
```



## handl_frontend_yaml::convert::yaml_to_json

*Function*

```rust
fn yaml_to_json(v: serde_yaml::Value) -> Result<serde_json::Value, crate::error::YamlError>
```



