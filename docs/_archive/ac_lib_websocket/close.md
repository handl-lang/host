**handl_lib_websocket > close**

# Module: close

## Contents

**Structs**

- [`CloseCode`](#closecode) - A WebSocket close code.
- [`CloseFrame`](#closeframe) - A parsed WebSocket close frame payload.

---

## handl_lib_websocket::close::CloseCode

*Struct*

A WebSocket close code.

**Tuple Struct**: `(u16)`

**Methods:**

- `fn from_be_bytes(bytes: [u8; 2]) -> Self` - Convert two big-endian bytes into a close code.
- `fn to_be_bytes(self: Self) -> [u8; 2]` - Encode this close code as two big-endian bytes.

**Traits:** Eq, Copy

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **From**
  - `fn from(code: u16) -> Self`
- **Hash**
  - `fn hash<__H>(self: &Self, state: & mut __H)`
- **PartialEq**
  - `fn eq(self: &Self, other: &CloseCode) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> CloseCode`



## handl_lib_websocket::close::CloseFrame

*Struct*

A parsed WebSocket close frame payload.

**Fields:**
- `code: CloseCode` - Status code.
- `reason: String` - Optional human-readable reason.

**Methods:**

- `fn new<impl Into<CloseCode>, impl Into<String>>(code: impl Trait, reason: impl Trait) -> Self` - Create a new close frame.
- `fn encode(self: &Self) -> Vec<u8>` - Encode a close frame payload: 2-byte code followed by UTF-8 reason.
- `fn decode(bytes: &[u8]) -> Result<Self, Error>` - Decode a close frame payload.

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> CloseFrame`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &CloseFrame) -> bool`



