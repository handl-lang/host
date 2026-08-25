**handl_lib_json**

# Module: handl_lib_json

## Contents

**Structs**

- [`Json`](#json) - Combined JSON codec.
- [`JsonDecoder`](#jsondecoder) - JSON decoder.
- [`JsonEncoder`](#jsonencoder) - JSON encoder.

---

## handl_lib_json::Json

*Struct*

Combined JSON codec.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Encoder**
  - `fn encode(self: &Self, value: &Value) -> Result<<Self as >::Output, SerializeError>`
- **Default**
  - `fn default() -> Json`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Decoder**
  - `fn decode(self: &Self, input: &str) -> Result<Value, SerializeError>`
- **Clone**
  - `fn clone(self: &Self) -> Json`



## handl_lib_json::JsonDecoder

*Struct*

JSON decoder.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Decoder**
  - `fn decode(self: &Self, input: &str) -> Result<Value, SerializeError>`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> JsonDecoder`
- **Clone**
  - `fn clone(self: &Self) -> JsonDecoder`



## handl_lib_json::JsonEncoder

*Struct*

JSON encoder.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> JsonEncoder`
- **Encoder**
  - `fn encode(self: &Self, value: &Value) -> Result<<Self as >::Output, SerializeError>`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> JsonEncoder`



