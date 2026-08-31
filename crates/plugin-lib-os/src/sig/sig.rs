use std::sync::Arc;

use handl_platform::Module;
use handl_platform::{EvalContext, EvalError, Function};

use crate::error::OsError;

/// Abstract signal service.
///
/// Implementations:
/// - `handl-lib-os-std` — backed by platform signal APIs
/// - [`sig_stub::StubSig`] — canned responses for testing
pub trait Sig: Send + Sync {
    fn send(&self, process_id: i64, signal: &str) -> Result<(), OsError>;
    fn list(&self) -> Result<Vec<String>, OsError>;
}

/// Module adapter for [`Sig`] — exposes `sig.*` functions.
pub struct SigModule {
    send: SendFn,
    list: ListFn,
}

impl SigModule {
    pub fn new(sig: impl Sig + 'static) -> Self {
        let sig = Arc::new(sig);
        SigModule {
            send: SendFn { sig: sig.clone() },
            list: ListFn { sig },
        }
    }
}

impl Module for SigModule {
    fn name(&self) -> &str {
        "sig"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "send" => Some(&self.send),
            "list" => Some(&self.list),
            _ => None,
        }
    }
}

struct SendFn {
    sig: Arc<dyn Sig>,
}

impl Function for SendFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if args.len() < 2 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                2,
                args.len(),
            )));
        }
        let process_id = args[0].as_f64().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))? as i64;
        let signal = args[1].as_str().ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "string", "non-string",
        )))?;
        self.sig.send(process_id, signal)?;
        Ok(serde_json::Value::Null)
    }
}

struct ListFn {
    sig: Arc<dyn Sig>,
}

impl Function for ListFn {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let signals = self.sig.list()?;
        let arr: Vec<serde_json::Value> =
            signals.into_iter().map(serde_json::Value::String).collect();
        Ok(serde_json::Value::from(serde_json::Value::Array(arr)))
    }
}
