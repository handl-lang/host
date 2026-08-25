**handl_typegen_backend_json_schema > emit**

# Module: emit

## Contents

**Functions**

- [`inject_schema_comment`](#inject_schema_comment) - Inject a `# yaml-language-server: $schema=...` comment at the top
- [`write_schema_file`](#write_schema_file) - Write a JSON Schema to a file.

---

## handl_typegen_backend_json_schema::emit::inject_schema_comment

*Function*

Inject a `# yaml-language-server: $schema=...` comment at the top
of a YAML file if not already present.

```rust
fn inject_schema_comment(yaml_path: &std::path::Path, schema_rel_path: &str) -> Result<(), String>
```



## handl_typegen_backend_json_schema::emit::write_schema_file

*Function*

Write a JSON Schema to a file.

Creates parent directories if needed. The output is pretty-printed
JSON with a trailing newline.

```rust
fn write_schema_file(schema: &serde_json::Value, path: &std::path::Path) -> Result<(), String>
```



