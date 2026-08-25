**handl_frontend_toml**

# Module: handl_frontend_toml

## Contents

**Modules**

- [`convert`](#convert)
- [`error`](#error)
- [`scanner`](#scanner) - TOML-specific source location scanner.

**Structs**

- [`TomlFrontend`](#tomlfrontend) - TOML frontend implementing the [`Frontend`] trait.

**Functions**

- [`from_toml`](#from_toml) - Converts a `toml::Value` into a core [`NonFunctionValue`].
- [`parse`](#parse) - Parses a TOML string into a [`NonFunctionValue`].
- [`to_string`](#to_string) - Serializes a [`NonFunctionValue`] to a TOML string.
- [`to_toml`](#to_toml) - Converts a core [`NonFunctionValue`] into a `toml::Value`.

---

## handl_frontend_toml::TomlFrontend

*Struct*

TOML frontend implementing the [`Frontend`] trait.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> TomlFrontend`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> TomlFrontend`
- **Frontend**
  - `fn parse(self: &Self, input: &str) -> Result<NonFunctionValue, <Self as >::Error>`
  - `fn serialize(self: &Self, value: &NonFunctionValue) -> Result<String, <Self as >::Error>`
  - `fn parse_with_locations(self: &Self, input: &str) -> Result<(NonFunctionValue, handl_source::SourceMap), <Self as >::Error>`



## Module: convert



## Module: error



## handl_frontend_toml::from_toml

*Function*

Converts a `toml::Value` into a core [`NonFunctionValue`].

```rust
fn from_toml(v: toml::Value) -> ac_runtime_values::NonFunctionValue
```



## handl_frontend_toml::parse

*Function*

Parses a TOML string into a [`NonFunctionValue`].

TOML is a frontend — it produces the same core [`NonFunctionValue`]
representation that the VM operates on. No execution logic
lives here.

# Example

```ignore
use handl_frontend_toml::parse;

let value = parse("name = \"handl\"\nversion = 1\n").unwrap();
assert_eq!(value.get("name").unwrap().as_str(), Some("handl"));
```

```rust
fn parse(input: &str) -> Result<ac_runtime_values::NonFunctionValue, TomlError>
```



## Module: scanner

TOML-specific source location scanner.

Walks a parsed `serde_json::Value` tree and scans the raw TOML text
to build a `SourceMap` mapping dotted paths to `SourceLocation`s.

TOML syntax differs from YAML/JSON:
- Keys use `=` not `:` (`key = value`)
- Nested tables use `[section]` or `[section.subsection]` headers
- Array of tables use `[[section]]` headers



## handl_frontend_toml::to_string

*Function*

Serializes a [`NonFunctionValue`] to a TOML string.

TOML requires a top-level table (object). If the value is not
an object, it is wrapped in a single-key table under `value`.

```rust
fn to_string(value: &ac_runtime_values::NonFunctionValue) -> Result<String, TomlError>
```



## handl_frontend_toml::to_toml

*Function*

Converts a core [`NonFunctionValue`] into a `toml::Value`.

```rust
fn to_toml(value: &ac_runtime_values::NonFunctionValue) -> toml::Value
```



