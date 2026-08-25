**handl_frontend_yaml**

# Module: handl_frontend_yaml

## Contents

**Modules**

- [`convert`](#convert)
- [`error`](#error)
- [`scanner`](#scanner) - YAML-specific source location scanner.

**Structs**

- [`YamlFrontend`](#yamlfrontend) - YAML frontend implementing the [`Frontend`] trait.

**Functions**

- [`from_yaml`](#from_yaml) - Converts a `serde_yaml::Value` into a core [`NonFunctionValue`].
- [`parse`](#parse) - Parses a YAML string into a [`NonFunctionValue`].
- [`to_string`](#to_string) - Serializes a [`NonFunctionValue`] to a YAML string.
- [`to_yaml`](#to_yaml) - Converts a core [`NonFunctionValue`] into a `serde_yaml::Value`.

---

## handl_frontend_yaml::YamlFrontend

*Struct*

YAML frontend implementing the [`Frontend`] trait.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> YamlFrontend`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> YamlFrontend`
- **Frontend**
  - `fn parse(self: &Self, input: &str) -> Result<NonFunctionValue, <Self as >::Error>`
  - `fn serialize(self: &Self, value: &NonFunctionValue) -> Result<String, <Self as >::Error>`
  - `fn parse_with_locations(self: &Self, input: &str) -> Result<(NonFunctionValue, handl_source::SourceMap), <Self as >::Error>`



## Module: convert



## Module: error



## handl_frontend_yaml::from_yaml

*Function*

Converts a `serde_yaml::Value` into a core [`NonFunctionValue`].

```rust
fn from_yaml(v: serde_yaml::Value) -> ac_runtime_values::NonFunctionValue
```



## handl_frontend_yaml::parse

*Function*

Parses a YAML string into a [`NonFunctionValue`].

YAML is a frontend — it produces the same core [`NonFunctionValue`]
representation that the VM operates on. No execution logic
lives here.

# Example

```ignore
use handl_frontend_yaml::parse;

let value = parse("name: handl\nversion: 1\n").unwrap();
assert_eq!(value.get("name").unwrap().as_str(), Some("handl"));
```

```rust
fn parse(input: &str) -> Result<ac_runtime_values::NonFunctionValue, YamlError>
```



## Module: scanner

YAML-specific source location scanner.

Walks a parsed `serde_json::Value` tree and scans the raw YAML text
to build a `SourceMap` mapping dotted paths to `SourceLocation`s.



## handl_frontend_yaml::to_string

*Function*

Serializes a [`NonFunctionValue`] to a YAML string.

```rust
fn to_string(value: &ac_runtime_values::NonFunctionValue) -> Result<String, YamlError>
```



## handl_frontend_yaml::to_yaml

*Function*

Converts a core [`NonFunctionValue`] into a `serde_yaml::Value`.

```rust
fn to_yaml(value: &ac_runtime_values::NonFunctionValue) -> serde_yaml::Value
```



