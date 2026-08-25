**handl_lib_websocket > reassembler**

# Module: reassembler

## Contents

**Structs**

- [`Reassembler`](#reassembler) - Reassembles fragmented WebSocket frames into complete [`Message`]s.

---

## handl_lib_websocket::reassembler::Reassembler

*Struct*

Reassembles fragmented WebSocket frames into complete [`Message`]s.

Control frames are yielded immediately and can interleave with fragmented
data frames.

**Fields:**
- `buffer: Vec<u8>`
- `start_opcode: Option<crate::OpCode>`

**Methods:**

- `fn new() -> Self` - Create a new, empty reassembler.
- `fn feed(self: & mut Self, frame: &Frame) -> Result<Option<Message>, Error>` - Feed a single frame and return any complete message.
- `fn reset(self: & mut Self)` - Reset the reassembler to its initial state.

**Trait Implementations:**

- **Default**
  - `fn default() -> Reassembler`
- **Clone**
  - `fn clone(self: &Self) -> Reassembler`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



