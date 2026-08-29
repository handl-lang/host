use super::*;

// ── OpCode ──────────────────────────────────────────────────────────

#[test]
fn opcode_roundtrip() {
    for code in [
        OpCode::Continuation,
        OpCode::Text,
        OpCode::Binary,
        OpCode::Close,
        OpCode::Ping,
        OpCode::Pong,
    ] {
        assert_eq!(OpCode::from_u8(code.as_u8()).unwrap(), code);
    }
}

#[test]
fn opcode_invalid() {
    assert!(matches!(
        OpCode::from_u8(0x0B),
        Err(Error::InvalidOpCode(0x0B))
    ));
    assert!(matches!(
        OpCode::from_u8(0xFF),
        Err(Error::InvalidOpCode(0xFF))
    ));
}

#[test]
fn opcode_is_control() {
    assert!(OpCode::Close.is_control());
    assert!(OpCode::Ping.is_control());
    assert!(OpCode::Pong.is_control());
    assert!(!OpCode::Text.is_control());
    assert!(!OpCode::Binary.is_control());
    assert!(!OpCode::Continuation.is_control());
}

#[test]
fn opcode_is_data() {
    assert!(OpCode::Text.is_data());
    assert!(OpCode::Binary.is_data());
    assert!(OpCode::Continuation.is_data());
    assert!(!OpCode::Close.is_data());
    assert!(!OpCode::Ping.is_data());
    assert!(!OpCode::Pong.is_data());
}

// ── Error ───────────────────────────────────────────────────────────

#[test]
fn error_display_and_source() {
    assert_eq!(
        format!("{}", Error::InvalidOpCode(0x0B)),
        "invalid websocket opcode: 11"
    );
    assert_eq!(
        format!("{}", Error::InvalidPayloadLength),
        "invalid payload length"
    );
    assert_eq!(
        format!("{}", Error::Incomplete),
        "incomplete websocket frame"
    );
    assert_eq!(
        format!("{}", Error::FragmentedControl),
        "fragmented or oversized control frame"
    );
    assert_eq!(
        format!("{}", Error::InvalidUtf8),
        "invalid utf-8 in text frame"
    );
    assert_eq!(
        format!("{}", Error::InvalidCloseFrame),
        "invalid close frame payload"
    );
    assert_eq!(
        format!("{}", Error::UnexpectedContinuation),
        "unexpected continuation frame"
    );
    assert_eq!(
        format!("{}", Error::ReservedBits),
        "reserved bits are not supported"
    );

    let io = Error::Io(std::io::Error::new(std::io::ErrorKind::Other, "boom"));
    assert!(std::error::Error::source(&io).is_some());
    assert!(std::error::Error::source(&Error::InvalidOpCode(0)).is_none());
}

// ── CloseCode / CloseFrame ──────────────────────────────────────────

#[test]
fn close_code_constants() {
    assert_eq!(CloseCode::NORMAL.0, 1000);
    assert_eq!(CloseCode::GOING_AWAY.0, 1001);
    assert_eq!(CloseCode::PROTOCOL_ERROR.0, 1002);
    assert_eq!(CloseCode::UNSUPPORTED_DATA.0, 1003);
    assert_eq!(CloseCode::MESSAGE_TOO_BIG.0, 1009);
    assert_eq!(CloseCode::TLS_HANDSHAKE.0, 1015);
}

#[test]
fn close_code_bytes_roundtrip() {
    let code = CloseCode::from(3000);
    assert_eq!(CloseCode::from_be_bytes(code.to_be_bytes()), code);
}

#[test]
fn close_frame_roundtrip() {
    let original = CloseFrame::new(CloseCode::NORMAL, "goodbye");
    let bytes = original.encode();
    assert_eq!(
        bytes,
        [0x03, 0xE8, b'g', b'o', b'o', b'd', b'b', b'y', b'e']
    );
    let decoded = CloseFrame::decode(&bytes).unwrap();
    assert_eq!(decoded, original);
}

#[test]
fn close_frame_empty_reason() {
    let frame = CloseFrame::new(CloseCode::NORMAL, "");
    assert_eq!(frame.encode(), [0x03, 0xE8]);
    let decoded = CloseFrame::decode(&[0x03, 0xE8]).unwrap();
    assert_eq!(decoded.reason, "");
}

#[test]
fn close_frame_too_short() {
    assert!(matches!(
        CloseFrame::decode(&[0x03]),
        Err(Error::InvalidCloseFrame)
    ));
}

#[test]
fn close_frame_invalid_utf8() {
    let bytes = vec![0x03, 0xE8, 0xC0, 0x80];
    assert!(matches!(
        CloseFrame::decode(&bytes),
        Err(Error::InvalidUtf8)
    ));
}

// ── Mask ────────────────────────────────────────────────────────────

#[test]
fn mask_double_apply_is_identity() {
    let key = [0x12, 0x34, 0x56, 0x78];
    let original = b"The quick brown fox jumps over the lazy dog".to_vec();
    let mut masked = original.clone();
    mask::apply(&mut masked, key);
    assert_ne!(masked, original);
    mask::apply(&mut masked, key);
    assert_eq!(masked, original);
}

#[test]
fn mask_empty_payload() {
    let mut payload = Vec::new();
    mask::apply(&mut payload, [1, 2, 3, 4]);
    assert!(payload.is_empty());
}

#[test]
fn mask_short_payload() {
    let mut payload = vec![0x00, 0x01, 0x02];
    mask::apply(&mut payload, [0xFF, 0xFF, 0xFF, 0xFF]);
    assert_eq!(payload, vec![0xFF, 0xFE, 0xFD]);
}

// ── Frame encoding / decoding ───────────────────────────────────────

#[test]
fn frame_roundtrip_text_unmasked() {
    let frame = Frame::new(OpCode::Text, b"hello".to_vec());
    let bytes = frame.encode();
    let (decoded, consumed) = Frame::decode(&bytes).unwrap();
    assert_eq!(consumed, bytes.len());
    assert_eq!(decoded, frame);
    assert_eq!(decoded.payload, b"hello");
}

#[test]
fn frame_roundtrip_text_masked() {
    let frame = Frame::new(OpCode::Text, b"hello".to_vec()).with_mask([0xAB, 0xCD, 0xEF, 0x01]);
    let bytes = frame.encode();
    let (decoded, consumed) = Frame::decode(&bytes).unwrap();
    assert_eq!(consumed, bytes.len());
    assert_eq!(decoded, frame);
    assert_eq!(decoded.payload, b"hello");
    assert_eq!(decoded.mask, Some([0xAB, 0xCD, 0xEF, 0x01]));
}

#[test]
fn frame_roundtrip_binary() {
    let payload: Vec<u8> = (0..=255).collect();
    let frame = Frame::new(OpCode::Binary, payload.clone());
    let bytes = frame.encode();
    let (decoded, _) = Frame::decode(&bytes).unwrap();
    assert_eq!(decoded.payload, payload);
}

#[test]
fn frame_roundtrip_ping() {
    let frame = Frame::new(OpCode::Ping, vec![0x01, 0x02, 0x03]);
    let bytes = frame.encode();
    let (decoded, _) = Frame::decode(&bytes).unwrap();
    assert_eq!(decoded, frame);
}

#[test]
fn frame_roundtrip_close() {
    let close = CloseFrame::new(CloseCode::NORMAL, "done");
    let frame = Frame::new(OpCode::Close, close.encode());
    let bytes = frame.encode();
    let (decoded, _) = Frame::decode(&bytes).unwrap();
    assert_eq!(decoded, frame);
}

#[test]
fn frame_small_payload_length_126() {
    let payload = vec![0xABu8; 200];
    let frame = Frame::new(OpCode::Binary, payload.clone()).with_mask([1, 2, 3, 4]);
    let bytes = frame.encode();
    // 2 base header + 2 extended length + 4 mask + 200 payload
    assert_eq!(bytes.len(), 2 + 2 + 4 + 200);
    let (decoded, _) = Frame::decode(&bytes).unwrap();
    assert_eq!(decoded.payload, payload);
}

#[test]
fn frame_large_payload_length_127() {
    let payload = vec![0xCDu8; 65536];
    let frame = Frame::new(OpCode::Binary, payload.clone());
    let bytes = frame.encode();
    // 2 base header + 8 extended length + 65536 payload (no mask)
    assert_eq!(bytes.len(), 2 + 8 + 65536);
    let (decoded, _) = Frame::decode(&bytes).unwrap();
    assert_eq!(decoded.payload, payload);
}

#[test]
fn frame_incomplete_header() {
    assert!(matches!(Frame::decode(&[]), Err(Error::Incomplete)));
    assert!(matches!(Frame::decode(&[0x81]), Err(Error::Incomplete)));
}

#[test]
fn frame_incomplete_126_length() {
    let bytes = [0x81, 0x7E, 0x00];
    assert!(matches!(Frame::decode(&bytes), Err(Error::Incomplete)));
}

#[test]
fn frame_incomplete_127_length() {
    let bytes = [0x81, 0x7F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    assert!(matches!(Frame::decode(&bytes), Err(Error::Incomplete)));
}

#[test]
fn frame_incomplete_payload() {
    let bytes = [0x81, 0x05, b'h', b'e', b'l', b'l'];
    assert!(matches!(Frame::decode(&bytes), Err(Error::Incomplete)));
}

#[test]
fn frame_incomplete_mask() {
    let bytes = [0x81, 0x85, b'h', b'e'];
    assert!(matches!(Frame::decode(&bytes), Err(Error::Incomplete)));
}

#[test]
fn frame_reserved_bits_rejected() {
    let frame = Frame::new(OpCode::Text, b"x".to_vec()).with_rsv(true, false, false);
    let bytes = frame.encode();
    assert!(matches!(Frame::decode(&bytes), Err(Error::ReservedBits)));
}

#[test]
fn frame_fragmented_control_rejected() {
    let payload = vec![0u8; 126];
    let frame = Frame::new(OpCode::Ping, payload);
    let bytes = frame.encode();
    assert!(matches!(
        Frame::decode(&bytes),
        Err(Error::FragmentedControl)
    ));
}

#[test]
fn frame_non_final_control_rejected() {
    let frame = Frame::new(OpCode::Ping, vec![]).with_fin(false);
    let bytes = frame.encode();
    assert!(matches!(
        Frame::decode(&bytes),
        Err(Error::FragmentedControl)
    ));
}

#[test]
fn frame_invalid_opcode_rejected() {
    let bytes = [0x8B, 0x80, 0x00, 0x00, 0x00, 0x00];
    assert!(matches!(
        Frame::decode(&bytes),
        Err(Error::InvalidOpCode(0x0B))
    ));
}

// ── Message ─────────────────────────────────────────────────────────

#[test]
fn message_text_roundtrip_unmasked() {
    let msg = Message::Text("hello".to_string());
    let frame = msg.into_frame(None);
    let (decoded, _) = Frame::decode(&frame.encode()).unwrap();
    let decoded_msg = Message::from_frame(&decoded).unwrap();
    assert_eq!(decoded_msg, Message::Text("hello".to_string()));
}

#[test]
fn message_text_roundtrip_masked() {
    let msg = Message::Text("hello".to_string());
    let frame = msg.into_frame(Some([0xFF, 0x00, 0xFF, 0x00]));
    let (decoded, _) = Frame::decode(&frame.encode()).unwrap();
    let decoded_msg = Message::from_frame(&decoded).unwrap();
    assert_eq!(decoded_msg, Message::Text("hello".to_string()));
}

#[test]
fn message_binary_roundtrip() {
    let payload = vec![0u8, 1, 2, 255];
    let msg = Message::Binary(payload);
    let frame = msg.clone().into_frame(None);
    let (decoded, _) = Frame::decode(&frame.encode()).unwrap();
    let decoded_msg = Message::from_frame(&decoded).unwrap();
    assert_eq!(decoded_msg, msg);
}

#[test]
fn message_ping_roundtrip() {
    let msg = Message::Ping(vec![0x01, 0x02]);
    let frame = msg.clone().into_frame(None);
    let (decoded, _) = Frame::decode(&frame.encode()).unwrap();
    let decoded_msg = Message::from_frame(&decoded).unwrap();
    assert_eq!(decoded_msg, msg);
}

#[test]
fn message_close_roundtrip_with_reason() {
    let msg = Message::Close(Some(CloseFrame::new(CloseCode::NORMAL, "done")));
    let frame = msg.clone().into_frame(None);
    let (decoded, _) = Frame::decode(&frame.encode()).unwrap();
    let decoded_msg = Message::from_frame(&decoded).unwrap();
    assert_eq!(decoded_msg, msg);
}

#[test]
fn message_close_roundtrip_no_reason() {
    let msg = Message::Close(None);
    let frame = msg.clone().into_frame(None);
    let (decoded, _) = Frame::decode(&frame.encode()).unwrap();
    let decoded_msg = Message::from_frame(&decoded).unwrap();
    assert_eq!(decoded_msg, msg);
}

#[test]
fn message_invalid_utf8_rejected() {
    let invalid = vec![0x80, 0x81, 0x82];
    let frame = Frame::new(OpCode::Text, invalid);
    assert!(matches!(
        Message::from_frame(&frame),
        Err(Error::InvalidUtf8)
    ));
}

#[test]
fn message_unfragmented_continuation_rejected() {
    let frame = Frame::new(OpCode::Continuation, b"x".to_vec());
    assert!(matches!(
        Message::from_frame(&frame),
        Err(Error::UnexpectedContinuation)
    ));
}

// ── Reassembler ─────────────────────────────────────────────────────

#[test]
fn reassembler_single_text() {
    let mut asm = Reassembler::new();
    let frame = Frame::new(OpCode::Text, b"hello".to_vec());
    let msg = asm.feed(&frame).unwrap();
    assert_eq!(msg, Some(Message::Text("hello".to_string())));
    assert!(asm.buffer.is_empty());
    assert!(asm.start_opcode.is_none());
}

#[test]
fn reassembler_fragmented_text() {
    let mut asm = Reassembler::new();
    let start = Frame::new(OpCode::Text, b"hel".to_vec()).with_fin(false);
    let cont = Frame::new(OpCode::Continuation, b"lo".to_vec());

    assert_eq!(asm.feed(&start).unwrap(), None);
    assert_eq!(
        asm.feed(&cont).unwrap(),
        Some(Message::Text("hello".to_string()))
    );
}

#[test]
fn reassembler_fragmented_binary() {
    let mut asm = Reassembler::new();
    let start = Frame::new(OpCode::Binary, vec![0, 1, 2]).with_fin(false);
    let cont = Frame::new(OpCode::Continuation, vec![3, 4, 5]);

    assert_eq!(asm.feed(&start).unwrap(), None);
    assert_eq!(
        asm.feed(&cont).unwrap(),
        Some(Message::Binary(vec![0, 1, 2, 3, 4, 5]))
    );
}

#[test]
fn reassembler_interleaved_control() {
    let mut asm = Reassembler::new();
    let start = Frame::new(OpCode::Text, b"he".to_vec()).with_fin(false);
    let ping = Frame::new(OpCode::Ping, vec![0x01]);
    let cont = Frame::new(OpCode::Continuation, b"llo".to_vec());

    assert_eq!(asm.feed(&start).unwrap(), None);
    assert_eq!(asm.feed(&ping).unwrap(), Some(Message::Ping(vec![0x01])));
    assert_eq!(
        asm.feed(&cont).unwrap(),
        Some(Message::Text("hello".to_string()))
    );
}

#[test]
fn reassembler_unexpected_continuation() {
    let mut asm = Reassembler::new();
    let cont = Frame::new(OpCode::Continuation, b"x".to_vec());
    assert!(matches!(
        asm.feed(&cont),
        Err(Error::UnexpectedContinuation)
    ));
}

#[test]
fn reassembler_unexpected_data_during_reassembly() {
    let mut asm = Reassembler::new();
    let start = Frame::new(OpCode::Text, b"he".to_vec()).with_fin(false);
    let bad = Frame::new(OpCode::Text, b"x".to_vec()).with_fin(false);

    assert_eq!(asm.feed(&start).unwrap(), None);
    assert!(matches!(asm.feed(&bad), Err(Error::UnexpectedContinuation)));
}

#[test]
fn reassembler_fragmented_control_rejected() {
    let mut asm = Reassembler::new();
    let ping = Frame::new(OpCode::Ping, vec![0x01]).with_fin(false);
    assert!(matches!(asm.feed(&ping), Err(Error::FragmentedControl)));
}

#[test]
fn reassembler_oversized_control_rejected() {
    let mut asm = Reassembler::new();
    let ping = Frame::new(OpCode::Ping, vec![0u8; 126]);
    assert!(matches!(asm.feed(&ping), Err(Error::FragmentedControl)));
}

mod module_helpers {
    use crate::module::{
        ClientClose, ClientConnect, ClientNew, ClientRecv, ClientSend, FrameDecode, FrameEncode,
        MessageFromFrame, MessageToFrame, ServerListen, ServerNew, ServerStop,
    };
    use handl_plugin_vm::Function;
    use handl_plugin_vm_values_json::{CanonicalContext, NonFunctionValue};

    fn ctx() -> CanonicalContext {
        CanonicalContext::new()
    }

    fn call_err<F: Function<NonFunctionValue>>(
        f: &F,
        args: &[NonFunctionValue],
        ctx: &mut CanonicalContext,
    ) {
        assert!(f.call(args, ctx).is_err());
    }

    fn call_ok<F: Function<NonFunctionValue>>(
        f: &F,
        args: &[NonFunctionValue],
        ctx: &mut CanonicalContext,
    ) -> NonFunctionValue {
        f.call(args, ctx).unwrap()
    }

    #[test]
    fn frame_encode_rejects_missing_opcode() {
        let mut ctx = ctx();
        call_err(
            &FrameEncode,
            &[NonFunctionValue::new(serde_json::json!({
                "payload": "aGVsbG8=",
            }))],
            &mut ctx,
        );
    }

    #[test]
    fn frame_encode_rejects_invalid_opcode() {
        let mut ctx = ctx();
        call_err(
            &FrameEncode,
            &[NonFunctionValue::new(serde_json::json!({
                "opcode": "foo",
                "payload": "aGVsbG8=",
            }))],
            &mut ctx,
        );
    }

    #[test]
    fn frame_encode_rejects_bad_base64_payload() {
        let mut ctx = ctx();
        call_err(
            &FrameEncode,
            &[NonFunctionValue::new(serde_json::json!({
                "opcode": "text",
                "payload": "not-base64",
            }))],
            &mut ctx,
        );
    }

    #[test]
    fn frame_encode_rejects_mask_non_string() {
        let mut ctx = ctx();
        call_err(
            &FrameEncode,
            &[NonFunctionValue::new(serde_json::json!({
                "opcode": "text",
                "payload": "aGVsbG8=",
                "mask": 123,
            }))],
            &mut ctx,
        );
    }

    #[test]
    fn frame_encode_rejects_bad_mask_length() {
        let mut ctx = ctx();
        call_err(
            &FrameEncode,
            &[NonFunctionValue::new(serde_json::json!({
                "opcode": "text",
                "payload": "aGVsbG8=",
                "mask": "aGVsbG8=",
            }))],
            &mut ctx,
        );
    }

    #[test]
    fn frame_encode_rejects_fin_non_bool() {
        let mut ctx = ctx();
        call_err(
            &FrameEncode,
            &[NonFunctionValue::new(serde_json::json!({
                "opcode": "text",
                "payload": "aGVsbG8=",
                "fin": "yes",
            }))],
            &mut ctx,
        );
    }

    #[test]
    fn frame_encode_rejects_non_object() {
        let mut ctx = ctx();
        call_err(
            &FrameEncode,
            &[NonFunctionValue::new(serde_json::json!("not an object"))],
            &mut ctx,
        );
    }

    #[test]
    fn frame_decode_rejects_non_base64() {
        let mut ctx = ctx();
        call_err(
            &FrameDecode,
            &[NonFunctionValue::new(serde_json::json!("not-base64!!!"))],
            &mut ctx,
        );
    }

    #[test]
    fn frame_decode_rejects_invalid_frame() {
        let mut ctx = ctx();
        call_err(
            &FrameDecode,
            &[NonFunctionValue::new(serde_json::json!("cAA="))],
            &mut ctx,
        );
    }

    #[test]
    fn message_to_frame_rejects_empty_object() {
        let mut ctx = ctx();
        call_err(
            &MessageToFrame,
            &[NonFunctionValue::new(serde_json::json!({}))],
            &mut ctx,
        );
    }

    #[test]
    fn message_to_frame_rejects_invalid_object() {
        let mut ctx = ctx();
        call_err(
            &MessageToFrame,
            &[NonFunctionValue::new(
                serde_json::json!({"unknown": "value"}),
            )],
            &mut ctx,
        );
    }

    #[test]
    fn message_to_frame_rejects_text_non_string() {
        let mut ctx = ctx();
        call_err(
            &MessageToFrame,
            &[NonFunctionValue::new(serde_json::json!({"text": 123}))],
            &mut ctx,
        );
    }

    #[test]
    fn message_to_frame_rejects_binary_non_string() {
        let mut ctx = ctx();
        call_err(
            &MessageToFrame,
            &[NonFunctionValue::new(serde_json::json!({"binary": 123}))],
            &mut ctx,
        );
    }

    #[test]
    fn message_to_frame_rejects_binary_bad_base64() {
        let mut ctx = ctx();
        call_err(
            &MessageToFrame,
            &[NonFunctionValue::new(
                serde_json::json!({"binary": "not-base64"}),
            )],
            &mut ctx,
        );
    }

    #[test]
    fn message_to_frame_rejects_close_non_object() {
        let mut ctx = ctx();
        call_err(
            &MessageToFrame,
            &[NonFunctionValue::new(serde_json::json!({"close": "bad"}))],
            &mut ctx,
        );
    }

    #[test]
    fn message_to_frame_rejects_close_missing_code() {
        let mut ctx = ctx();
        call_err(
            &MessageToFrame,
            &[NonFunctionValue::new(serde_json::json!({
                "close": {"reason": "nope"},
            }))],
            &mut ctx,
        );
    }

    #[test]
    fn message_to_frame_rejects_close_code_non_number() {
        let mut ctx = ctx();
        call_err(
            &MessageToFrame,
            &[NonFunctionValue::new(serde_json::json!({
                "close": {"code": "1000", "reason": "nope"},
            }))],
            &mut ctx,
        );
    }

    #[test]
    fn message_to_frame_rejects_close_code_fractional() {
        let mut ctx = ctx();
        call_err(
            &MessageToFrame,
            &[NonFunctionValue::new(serde_json::json!({
                "close": {"code": 1000.5, "reason": "nope"},
            }))],
            &mut ctx,
        );
    }

    #[test]
    fn message_to_frame_rejects_close_code_negative() {
        let mut ctx = ctx();
        call_err(
            &MessageToFrame,
            &[NonFunctionValue::new(serde_json::json!({
                "close": {"code": -1, "reason": "nope"},
            }))],
            &mut ctx,
        );
    }

    #[test]
    fn message_to_frame_rejects_close_code_too_large() {
        let mut ctx = ctx();
        call_err(
            &MessageToFrame,
            &[NonFunctionValue::new(serde_json::json!({
                "close": {"code": 70000, "reason": "nope"},
            }))],
            &mut ctx,
        );
    }

    #[test]
    fn message_from_frame_rejects_non_object() {
        let mut ctx = ctx();
        call_err(
            &MessageFromFrame,
            &[NonFunctionValue::new(serde_json::json!("not an object"))],
            &mut ctx,
        );
    }

    #[test]
    fn client_functions_require_arguments() {
        let mut ctx = ctx();
        call_err(&ClientConnect, &[], &mut ctx);
        call_err(&ClientSend, &[], &mut ctx);
        call_err(&ClientRecv, &[], &mut ctx);
        call_err(&ClientClose, &[], &mut ctx);
    }

    #[test]
    fn client_connect_rejects_non_string_url() {
        let mut ctx = ctx();
        let c = call_ok(&ClientNew, &[], &mut ctx);
        call_err(
            &ClientConnect,
            &[c, NonFunctionValue::new(serde_json::json!(123))],
            &mut ctx,
        );
    }

    #[test]
    fn client_recv_rejects_missing_client_id() {
        let mut ctx = ctx();
        call_err(
            &ClientRecv,
            &[NonFunctionValue::new(
                serde_json::json!({"type": "websocket_client"}),
            )],
            &mut ctx,
        );
    }

    #[test]
    fn client_recv_rejects_non_number_client_id() {
        let mut ctx = ctx();
        call_err(
            &ClientRecv,
            &[NonFunctionValue::new(
                serde_json::json!({"client_id": "foo"}),
            )],
            &mut ctx,
        );
    }

    #[test]
    fn server_functions_require_arguments() {
        let mut ctx = ctx();
        call_err(&ServerListen, &[], &mut ctx);
        call_err(&ServerStop, &[], &mut ctx);
    }

    #[test]
    fn frame_encode_roundtrip() {
        let mut ctx = ctx();
        let frame = call_ok(
            &FrameEncode,
            &[NonFunctionValue::new(serde_json::json!({
                "opcode": "text",
                "payload": "aGVsbG8=",
            }))],
            &mut ctx,
        );
        assert!(frame.inner().is_string());

        let round = call_ok(&FrameDecode, &[frame], &mut ctx);
        assert_eq!(
            round.inner().get("opcode"),
            Some(&serde_json::json!("text"))
        );
    }

    #[test]
    fn frame_encode_with_mask() {
        let mut ctx = ctx();
        call_ok(
            &FrameEncode,
            &[NonFunctionValue::new(serde_json::json!({
                "opcode": "text",
                "payload": "aGVsbG8=",
                "mask": "dGVzdA==",
            }))],
            &mut ctx,
        );
    }

    #[test]
    fn message_to_frame_text() {
        let mut ctx = ctx();
        let encoded = call_ok(
            &MessageToFrame,
            &[NonFunctionValue::new(serde_json::json!({"text": "hello"}))],
            &mut ctx,
        );
        assert!(encoded.inner().is_string());
    }

    #[test]
    fn message_to_frame_binary() {
        let mut ctx = ctx();
        let encoded = call_ok(
            &MessageToFrame,
            &[NonFunctionValue::new(
                serde_json::json!({"binary": "aGVsbG8="}),
            )],
            &mut ctx,
        );
        assert!(encoded.inner().is_string());
    }

    #[test]
    fn message_to_frame_ping_pong() {
        let mut ctx = ctx();
        call_ok(
            &MessageToFrame,
            &[NonFunctionValue::new(serde_json::json!({"ping": ""}))],
            &mut ctx,
        );
        call_ok(
            &MessageToFrame,
            &[NonFunctionValue::new(serde_json::json!({"pong": ""}))],
            &mut ctx,
        );
    }

    #[test]
    fn message_to_frame_close_null() {
        let mut ctx = ctx();
        let encoded = call_ok(
            &MessageToFrame,
            &[NonFunctionValue::new(serde_json::json!({"close": null}))],
            &mut ctx,
        );
        assert!(encoded.inner().is_string());
    }

    #[test]
    fn message_to_frame_close_with_code() {
        let mut ctx = ctx();
        call_ok(
            &MessageToFrame,
            &[NonFunctionValue::new(serde_json::json!({
                "close": {"code": 1000, "reason": "normal"},
            }))],
            &mut ctx,
        );
    }

    #[test]
    fn message_from_frame_text() {
        let mut ctx = ctx();
        let encoded = call_ok(
            &MessageToFrame,
            &[NonFunctionValue::new(serde_json::json!({"text": "hello"}))],
            &mut ctx,
        );
        let frame = call_ok(&FrameDecode, &[encoded], &mut ctx);
        let msg = call_ok(&MessageFromFrame, &[frame], &mut ctx);
        assert_eq!(msg.inner().get("text"), Some(&serde_json::json!("hello")));
    }

    #[test]
    fn message_from_frame_binary() {
        let mut ctx = ctx();
        let encoded = call_ok(
            &MessageToFrame,
            &[NonFunctionValue::new(
                serde_json::json!({"binary": "aGVsbG8="}),
            )],
            &mut ctx,
        );
        let frame = call_ok(&FrameDecode, &[encoded], &mut ctx);
        let msg = call_ok(&MessageFromFrame, &[frame], &mut ctx);
        assert!(msg.inner().get("binary").is_some());
    }

    #[test]
    fn client_new_and_connect() {
        let mut ctx = ctx();
        let c = call_ok(&ClientNew, &[], &mut ctx);
        call_ok(
            &ClientConnect,
            &[
                c,
                NonFunctionValue::new(serde_json::json!("ws://127.0.0.1:0")),
            ],
            &mut ctx,
        );
    }

    #[test]
    fn client_send_recv_close_no_op() {
        let mut ctx = ctx();
        let c = call_ok(&ClientNew, &[], &mut ctx);
        call_ok(
            &ClientSend,
            &[
                c.clone(),
                NonFunctionValue::new(serde_json::json!({"text": "hello"})),
            ],
            &mut ctx,
        );
        call_ok(&ClientRecv, &[c.clone()], &mut ctx);
        call_ok(&ClientClose, &[c], &mut ctx);
    }

    #[test]
    fn server_new_listen_stop_no_op() {
        let mut ctx = ctx();
        let s = call_ok(&ServerNew, &[], &mut ctx);
        let handler = NonFunctionValue::new(serde_json::json!({
            "params": [{"name": "conn", "schema": null}, {"name": "msg", "schema": null}],
            "body": "msg",
        }));
        call_ok(
            &ServerListen,
            &[
                s.clone(),
                NonFunctionValue::new(serde_json::json!("ws://127.0.0.1:0")),
                handler,
            ],
            &mut ctx,
        );
        call_ok(&ServerStop, &[s], &mut ctx);
    }

    #[test]
    fn frame_encode_close_ping_pong() {
        let mut ctx = ctx();
        for opcode in ["close", "ping", "pong"] {
            call_ok(
                &FrameEncode,
                &[NonFunctionValue::new(serde_json::json!({
                    "opcode": opcode,
                    "payload": "",
                }))],
                &mut ctx,
            );
        }
    }

    #[test]
    fn frame_encode_with_fin() {
        let mut ctx = ctx();
        call_ok(
            &FrameEncode,
            &[NonFunctionValue::new(serde_json::json!({
                "opcode": "text",
                "payload": "aGVsbG8=",
                "fin": false,
            }))],
            &mut ctx,
        );
    }

    #[test]
    fn frame_decode_masked() {
        let mut ctx = ctx();
        let encoded = call_ok(
            &FrameEncode,
            &[NonFunctionValue::new(serde_json::json!({
                "opcode": "text",
                "payload": "aGVsbG8=",
                "mask": "dGVzdA==",
            }))],
            &mut ctx,
        );
        let decoded = call_ok(&FrameDecode, &[encoded], &mut ctx);
        assert!(decoded.inner().get("mask").is_some());
    }

    #[test]
    fn message_from_frame_ping_pong() {
        let mut ctx = ctx();
        for opcode in ["ping", "pong"] {
            let encoded = call_ok(
                &MessageToFrame,
                &[NonFunctionValue::new(serde_json::json!({
                    opcode: "",
                }))],
                &mut ctx,
            );
            let frame = call_ok(&FrameDecode, &[encoded], &mut ctx);
            call_ok(&MessageFromFrame, &[frame], &mut ctx);
        }
    }

    #[test]
    fn message_from_frame_close() {
        let mut ctx = ctx();
        let encoded = call_ok(
            &MessageToFrame,
            &[NonFunctionValue::new(serde_json::json!({
                "close": null,
            }))],
            &mut ctx,
        );
        let frame = call_ok(&FrameDecode, &[encoded], &mut ctx);
        call_ok(&MessageFromFrame, &[frame], &mut ctx);

        let encoded2 = call_ok(
            &MessageToFrame,
            &[NonFunctionValue::new(serde_json::json!({
                "close": {"code": 1000, "reason": "normal"},
            }))],
            &mut ctx,
        );
        let frame2 = call_ok(&FrameDecode, &[encoded2], &mut ctx);
        call_ok(&MessageFromFrame, &[frame2], &mut ctx);
    }

    #[test]
    fn message_from_frame_rejects_invalid_utf8_text() {
        let mut ctx = ctx();
        let encoded = call_ok(
            &FrameEncode,
            &[NonFunctionValue::new(serde_json::json!({
                "opcode": "text",
                "payload": "gIE=",
            }))],
            &mut ctx,
        );
        let frame = call_ok(&FrameDecode, &[encoded], &mut ctx);
        call_err(&MessageFromFrame, &[frame], &mut ctx);
    }

    #[test]
    fn client_new_rejects_arguments() {
        let mut ctx = ctx();
        call_err(
            &ClientNew,
            &[NonFunctionValue::new(serde_json::json!(null))],
            &mut ctx,
        );
    }

    #[test]
    fn server_new_rejects_arguments() {
        let mut ctx = ctx();
        call_err(
            &ServerNew,
            &[NonFunctionValue::new(serde_json::json!(null))],
            &mut ctx,
        );
    }

    #[test]
    fn client_connect_rejects_raw_number_without_client() {
        let mut ctx = ctx();
        call_err(
            &ClientConnect,
            &[
                NonFunctionValue::new(serde_json::json!(99999)),
                NonFunctionValue::new(serde_json::json!("ws://127.0.0.1:0")),
            ],
            &mut ctx,
        );
    }
}
