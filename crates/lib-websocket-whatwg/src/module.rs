//! The `websocket_whatwg` handl runtime module.
//!
//! This is the browser-side WebSocket client counterpart to `websocket_rfc`.
//!
//! Currently it exposes a single `websocket_whatwg` token; the actual
//! `connect`/`send`/`recv`/`close` functions will be added as the browser
//! host implementation is built out.

use handl_vm::{Context, Function};
use handl_vm_common::Module;
use handl_vm_values_json::NonFunctionValue;
use serde_json::json;

/// The `websocket_whatwg` runtime module.
#[derive(Debug, Clone, Copy)]
pub struct WebsocketWhatwgModule;

impl Module for WebsocketWhatwgModule {
    fn name(&self) -> &str {
        "websocket_whatwg"
    }

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        match name {
            "websocket_whatwg" => Some(&WebsocketWhatwgName),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct WebsocketWhatwgName;

impl Function<NonFunctionValue> for WebsocketWhatwgName {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, handl_vm::Error<NonFunctionValue>> {
        if !args.is_empty() {
            return Err(handl_vm::Error::ArgumentCount {
                expected: 0,
                got: args.len(),
            });
        }

        Ok(NonFunctionValue::new(json!({
            "type": "websocket",
            "name": "websocket_whatwg",
        })))
    }
}
