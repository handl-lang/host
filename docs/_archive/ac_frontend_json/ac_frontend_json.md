**handl_frontend_json**

# Module: handl_frontend_json

## Contents

**Modules**

- [`scanner`](#scanner) - JSON-specific source location scanner.

**Structs**

- [`JsonFrontend`](#jsonfrontend) - JSON frontend implementing the [`Frontend`] trait.

**Enums**

- [`JsonError`](#jsonerror) - Errors that can occur during JSON parsing.

**Functions**

- [`from_json`](#from_json) - Converts a `serde_json::Value` into a core [`NonFunctionValue`].
- [`parse`](#parse) - Parses a JSON string into a [`NonFunctionValue`].
- [`to_json`](#to_json) - Converts a core [`NonFunctionValue`] into a `serde_json::Value`.
- [`to_string`](#to_string) - Serializes a [`NonFunctionValue`] to a JSON string.
- [`to_string_pretty`](#to_string_pretty) - Serializes a [`NonFunctionValue`] to a pretty-printed JSON string.

---

## handl_frontend_json::JsonError

*Enum*

Errors that can occur during JSON parsing.

**Variants:**
- `Parse(serde_json::Error)` - The underlying JSON parser returned an error.

**Trait Implementations:**

- **Error**
  - `fn source(self: &Self) -> ::core::option::Option<&dyn ::thiserror::__private20::Error>`
- **From**
  - `fn from(source: serde_json::Error) -> Self`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Display**
  - `fn fmt(self: &Self, __formatter: & mut ::core::fmt::Formatter) -> ::core::fmt::Result`



## handl_frontend_json::JsonFrontend

*Struct*

JSON frontend implementing the [`Frontend`] trait.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> JsonFrontend`
- **Frontend**
  - `fn parse(self: &Self, input: &str) -> Result<NonFunctionValue, <Self as >::Error>`
  - `fn serialize(self: &Self, value: &NonFunctionValue) -> Result<String, <Self as >::Error>`
  - `fn parse_with_locations(self: &Self, input: &str) -> Result<(NonFunctionValue, handl_source::SourceMap), <Self as >::Error>`
- **Clone**
  - `fn clone(self: &Self) -> JsonFrontend`



## handl_frontend_json::from_json

*Function*

Converts a `serde_json::Value` into a core [`NonFunctionValue`].

```rust
fn from_json(v: serde_json::Value) -> ac_runtime_values::NonFunctionValue
```



## handl_frontend_json::parse

*Function*

Parses a JSON string into a [`NonFunctionValue`].

JSON is a frontend — it produces the same core [`NonFunctionValue`]
representation that the VM operates on. No execution logic
lives here.

# Example

```ignore
use handl_frontend_json::parse;

let value = parse(r#"{"name": "handl", "version": 1}"#).unwrap();
assert_eq!(value.get("name").unwrap().as_str(), Some("handl"));
```

```rust
fn parse(input: &str) -> Result<ac_runtime_values::NonFunctionValue, JsonError>
```



## Module: scanner

JSON-specific source location scanner.

Walks a parsed `serde_json::Value` tree and scans the raw JSON text
to build a `SourceMap` mapping dotted paths to `SourceLocation`s.



## handl_frontend_json::to_json

*Function*

Converts a core [`NonFunctionValue`] into a `serde_json::Value`.

```rust
fn to_json(value: &ac_runtime_values::NonFunctionValue) -> serde_json::Value
```



## handl_frontend_json::to_string

*Function*

Serializes a [`NonFunctionValue`] to a JSON string.

```rust
fn to_string(value: &ac_runtime_values::NonFunctionValue) -> Result<String, JsonError>
```



## handl_frontend_json::to_string_pretty

*Function*

Serializes a [`NonFunctionValue`] to a pretty-printed JSON string.

```rust
fn to_string_pretty(value: &ac_runtime_values::NonFunctionValue) -> Result<String, JsonError>
```



