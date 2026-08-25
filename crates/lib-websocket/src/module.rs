//! The `websocket` handl runtime module.
//!
//! Exposes WebSocket frame and message helpers callable from handl
//! expressions:
//!
//! - `websocket.frame.encode(object)` — `base64` frame bytes
//! - `websocket.frame.decode(base64)` — frame object
//! - `websocket.message.from_frame(frame)` — message object
//! - `websocket.message.to_frame(message)` — `base64` frame bytes

use base64::{engine::Engine as _, prelude::BASE64_STANDARD};
use handl_vm::{Context, Function};
use handl_vm_common::Module;
use handl_vm_values_json::{Error, NonFunctionValue};
use serde_json::Value as JsonValue;

use crate::{CloseCode, CloseFrame, Frame, Message, OpCode};

/// The `websocket` module — WebSocket frame and message primitives.
#[derive(Clone, Copy)]
pub struct WebsocketModule;

impl Module for WebsocketModule {
    fn name(&self) -> &str {
        "websocket_rfc"
    }

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        match name {
            "frame.encode" => Some(&FrameEncode),
            "frame.decode" => Some(&FrameDecode),
            "message.from_frame" => Some(&MessageFromFrame),
            "message.to_frame" => Some(&MessageToFrame),
            _ => None,
        }
    }
}

fn parse_opcode(s: &str) -> Result<OpCode, Error> {
    match s {
        "text" => Ok(OpCode::Text),
        "binary" => Ok(OpCode::Binary),
        "close" => Ok(OpCode::Close),
        "ping" => Ok(OpCode::Ping),
        "pong" => Ok(OpCode::Pong),
        _ => Err(Error::PrimitiveError(format!("unknown websocket opcode: {s}"))),
    }
}

fn opcode_to_string(op: OpCode) -> &'static str {
    match op {
        OpCode::Text => "text",
        OpCode::Binary => "binary",
        OpCode::Close => "close",
        OpCode::Ping => "ping",
        OpCode::Pong => "pong",
        OpCode::Continuation => "continuation",
    }
}

fn build_frame(obj: &serde_json::Map<String, JsonValue>) -> Result<Frame, Error> {
    let opcode = obj
        .get("opcode")
        .and_then(|v| v.as_str())
        .ok_or(Error::TypeMismatch {
            expected: "string",
            got: "non-string",
        })
        .and_then(parse_opcode)?;

    let payload_b64 = obj.get("payload").and_then(|v| v.as_str()).unwrap_or("");
    let payload = BASE64_STANDARD
        .decode(payload_b64)
        .map_err(|e| Error::PrimitiveError(format!("base64 decode error: {e}")))?;

    let fin = obj
        .get("fin")
        .map(|v| {
            v.as_bool().ok_or(Error::TypeMismatch {
                expected: "bool",
                got: "non-bool",
            })
        })
        .unwrap_or(Ok(true))?;

    let mask = match obj.get("mask") {
        Some(v) if v.is_null() => None,
        Some(v) => {
            let s = v.as_str().ok_or(Error::TypeMismatch {
                expected: "string",
                got: "non-string",
            })?;
            let bytes = BASE64_STANDARD
                .decode(s)
                .map_err(|e| Error::PrimitiveError(format!("mask base64 decode error: {e}")))?;
            let bytes: [u8; 4] = bytes.try_into().map_err(|_| {
                Error::PrimitiveError("mask must be exactly 4 bytes".to_string())
            })?;
            Some(bytes)
        }
        None => None,
    };

    Ok(Frame::new(opcode, payload).with_fin(fin).with_mask_opt(mask))
}

fn require_one_object(args: &[NonFunctionValue]) -> Result<&serde_json::Map<String, JsonValue>, Error> {
    let arg = args.first().ok_or(Error::ArgumentCount {
        expected: 1,
        got: args.len(),
    })?;
    arg.as_object().ok_or(Error::TypeMismatch {
        expected: "object",
        got: "non-object",
    })
}

fn require_one_string(args: &[NonFunctionValue]) -> Result<&str, Error> {
    let arg = args.first().ok_or(Error::ArgumentCount {
        expected: 1,
        got: args.len(),
    })?;
    arg.as_str().ok_or(Error::TypeMismatch {
        expected: "string",
        got: "non-string",
    })
}

fn parse_u16(value: &JsonValue) -> Result<u16, Error> {
    let n = value.as_f64().ok_or(Error::TypeMismatch {
        expected: "number",
        got: "non-number",
    })?;
    if n < 0.0 || n.fract() != 0.0 || n > u16::MAX as f64 {
        return Err(Error::TypeMismatch {
            expected: "u16",
            got: "invalid integer",
        });
    }
    Ok(n as u16)
}

#[derive(Clone, Copy)]
struct FrameEncode;

impl Function<NonFunctionValue> for FrameEncode {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let obj = require_one_object(args)?;
        let frame = build_frame(obj)?;
        Ok(NonFunctionValue::new(BASE64_STANDARD.encode(&frame.encode())))
    }
}

#[derive(Clone, Copy)]
struct FrameDecode;

impl Function<NonFunctionValue> for FrameDecode {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let s = require_one_string(args)?;
        let bytes = BASE64_STANDARD
            .decode(s)
            .map_err(|e| Error::PrimitiveError(format!("base64 decode error: {e}")))?;
        let (frame, _) = Frame::decode(&bytes)
            .map_err(|e| Error::PrimitiveError(format!("frame decode error: {e}")))?;

        let mut obj = serde_json::Map::new();
        obj.insert("opcode".to_string(), JsonValue::String(opcode_to_string(frame.opcode).to_string()));
        obj.insert(
            "payload".to_string(),
            JsonValue::String(BASE64_STANDARD.encode(&frame.payload)),
        );
        obj.insert("fin".to_string(), JsonValue::Bool(frame.fin));
        obj.insert("rsv1".to_string(), JsonValue::Bool(frame.rsv1));
        obj.insert("rsv2".to_string(), JsonValue::Bool(frame.rsv2));
        obj.insert("rsv3".to_string(), JsonValue::Bool(frame.rsv3));
        if let Some(key) = frame.mask {
            obj.insert(
                "mask".to_string(),
                JsonValue::String(BASE64_STANDARD.encode(&key)),
            );
        }
        Ok(NonFunctionValue::new(JsonValue::Object(obj)))
    }
}

#[derive(Clone, Copy)]
struct MessageFromFrame;

impl Function<NonFunctionValue> for MessageFromFrame {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let obj = require_one_object(args)?;
        let frame = build_frame(obj)?;
        let message = Message::from_frame(&frame)
            .map_err(|e| Error::PrimitiveError(format!("message from frame error: {e}")))?;

        let value = match message {
            Message::Text(text) => serde_json::json!({ "text": text }),
            Message::Binary(bytes) => serde_json::json!({ "binary": BASE64_STANDARD.encode(&bytes) }),
            Message::Ping(bytes) => serde_json::json!({ "ping": BASE64_STANDARD.encode(&bytes) }),
            Message::Pong(bytes) => serde_json::json!({ "pong": BASE64_STANDARD.encode(&bytes) }),
            Message::Close(Some(frame)) => {
                serde_json::json!({ "close": { "code": frame.code.0, "reason": frame.reason } })
            }
            Message::Close(None) => serde_json::json!({ "close": null }),
        };
        Ok(NonFunctionValue::new(value))
    }
}

#[derive(Clone, Copy)]
struct MessageToFrame;

impl Function<NonFunctionValue> for MessageToFrame {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error> {
        let obj = require_one_object(args)?;
        if obj.len() != 1 {
            return Err(Error::PrimitiveError(
                "message object must have exactly one key".to_string(),
            ));
        }

        let key = obj.keys().next().unwrap();
        let value = obj.get(key).unwrap();
        let message = match key.as_str() {
            "text" => {
                let text = value.as_str().ok_or(Error::TypeMismatch {
                    expected: "string",
                    got: "non-string",
                })?;
                Message::Text(text.to_string())
            }
            "binary" => {
                let s = value.as_str().ok_or(Error::TypeMismatch {
                    expected: "string",
                    got: "non-string",
                })?;
                let bytes = BASE64_STANDARD
                    .decode(s)
                    .map_err(|e| Error::PrimitiveError(format!("base64 decode error: {e}")))?;
                Message::Binary(bytes)
            }
            "ping" => {
                let s = value.as_str().ok_or(Error::TypeMismatch {
                    expected: "string",
                    got: "non-string",
                })?;
                let bytes = BASE64_STANDARD
                    .decode(s)
                    .map_err(|e| Error::PrimitiveError(format!("base64 decode error: {e}")))?;
                Message::Ping(bytes)
            }
            "pong" => {
                let s = value.as_str().ok_or(Error::TypeMismatch {
                    expected: "string",
                    got: "non-string",
                })?;
                let bytes = BASE64_STANDARD
                    .decode(s)
                    .map_err(|e| Error::PrimitiveError(format!("base64 decode error: {e}")))?;
                Message::Pong(bytes)
            }
            "close" => {
                if value.is_null() {
                    Message::Close(None)
                } else {
                    let close_obj = value.as_object().ok_or(Error::TypeMismatch {
                        expected: "object",
                        got: "non-object",
                    })?;
                    let code = parse_u16(close_obj.get("code").ok_or(Error::TypeMismatch {
                        expected: "number",
                        got: "missing",
                    })?)?;
                    let reason = close_obj
                        .get("reason")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    Message::Close(Some(CloseFrame::new(CloseCode(code), reason)))
                }
            }
            _ => {
                return Err(Error::PrimitiveError(format!(
                    "unknown message type: {key}"
                )))
            }
        };

        let frame = message.into_frame(None);
        Ok(NonFunctionValue::new(BASE64_STANDARD.encode(&frame.encode())))
    }
}
