# handl_lib_websocket

WebSocket frame and message codec.

This crate provides a fast, dependency-free implementation of the WebSocket
framing protocol (RFC 6455). It is designed to be embedded in clients,
servers, and runtime modules that need to produce or consume WebSocket
frames without committing to a particular async runtime or TLS stack.

# Example

```
use handl_lib_websocket::{Frame, Message, OpCode};

let frame = Frame::new(OpCode::Text, b"hello".to_vec());
let bytes = frame.encode();
let (decoded, _) = Frame::decode(&bytes).unwrap();
let message = Message::from_frame(&decoded).unwrap();
assert_eq!(message, Message::Text("hello".to_string()));
```

## Modules

### [`handl_lib_websocket`](handl_lib_websocket.md)

*7 modules*

### [`close`](close.md)

*2 structs*

### [`error`](error.md)

*1 enum*

### [`frame`](frame.md)

*1 struct*

### [`mask`](mask.md)

*1 function*

### [`message`](message.md)

*1 enum*

### [`opcode`](opcode.md)

*1 enum*

### [`reassembler`](reassembler.md)

*1 struct*

