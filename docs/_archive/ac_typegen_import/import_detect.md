**handl_typegen_import > import_detect**

# Module: import_detect

## Contents

**Enums**

- [`ImportKind`](#importkind) - The kind of import detected in a field's `expr`.

**Functions**

- [`detect_import_path`](#detect_import_path) - Detect import patterns in an expression string.
- [`extract_data_path`](#extract_data_path) - Extract the data value for a data-driven import field.

---

## handl_typegen_import::import_detect::ImportKind

*Enum*

The kind of import detected in a field's `expr`.

**Variants:**
- `Static(String)` - `import('path/to/file.yml')` — static literal path.
- `DataDriven` - `import(arg)` — path comes from the field's data value.
- `None` - No import detected, or dynamic expression that can't be resolved statically.

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &ImportKind) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> ImportKind`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_typegen_import::import_detect::detect_import_path

*Function*

Detect import patterns in an expression string.

Recognized patterns:
- `import('...')` or `import("...")` → `ImportKind::Static(path)`
- `import(arg)` → `ImportKind::DataDriven`
- Anything else → `ImportKind::None`

```rust
fn detect_import_path(expr: &str) -> ImportKind
```



## handl_typegen_import::import_detect::extract_data_path

*Function*

Extract the data value for a data-driven import field.

Given the document data and a field name, returns the string value
of that field if it's a string. Used to resolve `import(arg)` paths.

```rust
fn extract_data_path(data: &serde_json::Value, field_name: &str) -> Option<String>
```



