**handl_lib_websocket > frame**

# Module: frame

## Contents

**Structs**

- [`Frame`](#frame) - A single WebSocket frame.

---

## handl_lib_websocket::frame::Frame

*Struct*

A single WebSocket frame.

The payload is always stored unmasked, regardless of whether `mask`
is set.

**Fields:**
- `fin: bool` - FIN bit: `true` if this is the final fragment.
- `rsv1: bool` - RSV1 bit.
- `rsv2: bool` - RSV2 bit.
- `rsv3: bool` - RSV3 bit.
- `opcode: crate::OpCode` - Frame opcode.
- `mask: Option<[u8; 4]>` - Masking key when the frame is masked. The payload is unmasked.
- `payload: Vec<u8>` - Unmasked payload bytes.

**Methods:**

- `fn new(opcode: OpCode, payload: Vec<u8>) -> Self` - Create a new, final, unmasked frame.
- `fn with_fin(self: Self, fin: bool) -> Self` - Set the FIN bit.
- `fn with_mask(self: Self, key: [u8; 4]) -> Self` - Set the masking key.
- `fn with_mask_opt(self: Self, key: Option<[u8; 4]>) -> Self` - Set the masking key from an `Option`.
- `fn with_rsv(self: Self, rsv1: bool, rsv2: bool, rsv3: bool) -> Self` - Set the reserved bits.
- `fn wire_size(self: &Self) -> usize` - Return the total wire size of this frame in bytes.
- `fn payload_len_bytes(self: &Self) -> usize`
- `fn encode(self: &Self) -> Vec<u8>` - Encode this frame into a byte vector.
- `fn decode(input: &[u8]) -> Result<(Self, usize), Error>` - Decode a single frame from a byte slice.

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &Frame) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> Frame`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



