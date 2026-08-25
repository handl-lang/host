**handl_lib_toml**

# Module: handl_lib_toml

## Contents

**Structs**

- [`Toml`](#toml) - Combined TOML codec.
- [`TomlDecoder`](#tomldecoder) - TOML decoder.
- [`TomlEncoder`](#tomlencoder) - TOML encoder.

---

## handl_lib_toml::Toml

*Struct*

Combined TOML codec.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Encoder**
  - `fn encode(self: &Self, value: &Value) -> Result<<Self as >::Output, SerializeError>`
- **Default**
  - `fn default() -> Toml`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Decoder**
  - `fn decode(self: &Self, input: &str) -> Result<Value, SerializeError>`
- **Clone**
  - `fn clone(self: &Self) -> Toml`



## handl_lib_toml::TomlDecoder

*Struct*

TOML decoder.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Decoder**
  - `fn decode(self: &Self, input: &str) -> Result<Value, SerializeError>`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> TomlDecoder`
- **Clone**
  - `fn clone(self: &Self) -> TomlDecoder`



## handl_lib_toml::TomlEncoder

*Struct*

TOML encoder.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> TomlEncoder`
- **Encoder**
  - `fn encode(self: &Self, value: &Value) -> Result<<Self as >::Output, SerializeError>`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> TomlEncoder`



