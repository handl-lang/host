**handl_lib_yaml**

# Module: handl_lib_yaml

## Contents

**Structs**

- [`Yaml`](#yaml) - Combined YAML codec.
- [`YamlDecoder`](#yamldecoder) - YAML decoder.
- [`YamlEncoder`](#yamlencoder) - YAML encoder.

---

## handl_lib_yaml::Yaml

*Struct*

Combined YAML codec.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Encoder**
  - `fn encode(self: &Self, value: &Value) -> Result<<Self as >::Output, SerializeError>`
- **Default**
  - `fn default() -> Yaml`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Decoder**
  - `fn decode(self: &Self, input: &str) -> Result<Value, SerializeError>`
- **Clone**
  - `fn clone(self: &Self) -> Yaml`



## handl_lib_yaml::YamlDecoder

*Struct*

YAML decoder.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Decoder**
  - `fn decode(self: &Self, input: &str) -> Result<Value, SerializeError>`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> YamlDecoder`
- **Clone**
  - `fn clone(self: &Self) -> YamlDecoder`



## handl_lib_yaml::YamlEncoder

*Struct*

YAML encoder.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> YamlEncoder`
- **Encoder**
  - `fn encode(self: &Self, value: &Value) -> Result<<Self as >::Output, SerializeError>`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> YamlEncoder`



