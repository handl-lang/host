**handl_lib_websocket > opcode**

# Module: opcode

## Contents

**Enums**

- [`OpCode`](#opcode) - WebSocket opcode as defined by RFC 6455.

---

## handl_lib_websocket::opcode::OpCode

*Enum*

WebSocket opcode as defined by RFC 6455.

**Variants:**
- `Continuation` - Continuation frame.
- `Text` - Text frame.
- `Binary` - Binary frame.
- `Close` - Close frame.
- `Ping` - Ping frame.
- `Pong` - Pong frame.

**Methods:**

- `fn from_u8(value: u8) -> Result<Self, Error>` - Convert an opcode byte into an `OpCode`.
- `fn as_u8(self: Self) -> u8` - Return the wire representation of this opcode.
- `fn is_control(self: Self) -> bool` - `true` for control opcodes (`Close`, `Ping`, `Pong`).
- `fn is_data(self: Self) -> bool` - `true` for data opcodes (`Text`, `Binary`, `Continuation`).

**Traits:** Eq, Copy

**Trait Implementations:**

- **Hash**
  - `fn hash<__H>(self: &Self, state: & mut __H)`
- **PartialEq**
  - `fn eq(self: &Self, other: &OpCode) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> OpCode`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



