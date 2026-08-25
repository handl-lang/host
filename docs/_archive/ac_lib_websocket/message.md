**handl_lib_websocket > message**

# Module: message

## Contents

**Enums**

- [`Message`](#message) - A complete WebSocket message.

---

## handl_lib_websocket::message::Message

*Enum*

A complete WebSocket message.

**Variants:**
- `Text(String)` - A text message.
- `Binary(Vec<u8>)` - A binary message.
- `Ping(Vec<u8>)` - A ping message.
- `Pong(Vec<u8>)` - A pong message.
- `Close(Option<crate::CloseFrame>)` - A close message with an optional close frame.

**Methods:**

- `fn from_frame(frame: &Frame) -> Result<Self, Error>` - Convert a single, unfragmented frame into a message.
- `fn into_frame(self: Self, mask: Option<[u8; 4]>) -> Frame` - Convert this message into a single frame.

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &Message) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> Message`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



