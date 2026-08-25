**handl_lib_websocket > error**

# Module: error

## Contents

**Enums**

- [`Error`](#error) - Errors that can occur when encoding or decoding WebSocket frames.

---

## handl_lib_websocket::error::Error

*Enum*

Errors that can occur when encoding or decoding WebSocket frames.

**Variants:**
- `Io(io::Error)` - Underlying I/O error.
- `InvalidOpCode(u8)` - An unknown opcode was encountered.
- `InvalidPayloadLength` - A payload length field is malformed or reserved.
- `Incomplete` - Not enough bytes were available to complete the operation.
- `FragmentedControl` - A control frame was fragmented or exceeded the maximum control payload.
- `InvalidUtf8` - Text payload is not valid UTF-8.
- `InvalidCloseFrame` - A close frame payload is malformed.
- `UnexpectedContinuation` - A continuation frame arrived without a starting text/binary frame.
- `ReservedBits` - One or more reserved bits were set and are not supported.

**Trait Implementations:**

- **From**
  - `fn from(value: io::Error) -> Self`
- **Error**
  - `fn source(self: &Self) -> Option<&dyn error::Error>`
- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



