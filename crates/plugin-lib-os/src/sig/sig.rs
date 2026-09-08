use std::sync::Arc;

use handl_platform::Module;
use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::{HandlModule, OpRegistry};

use crate::error::OsError;

/// The handl source declaring the os `op` interface.
const OS_HNL: &str = include_str!("../../../../handl-packages/os/os.hnl");

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
///
/// The interface is declared in `os.hnl` via `op`/`handl` declarations
/// and loaded via the generic [`HandlModule`]; this struct delegates
/// [`Module`] lookups to the [`HandlModule`] built from the service.
pub struct SigModule {
    module: HandlModule,
}

impl SigModule {
    pub fn new(sig: impl Sig + 'static) -> Self {
        let mut registry = OpRegistry::new();
        register_sig_ops(&mut registry, sig);
        Self {
            module: HandlModule::new("sig", OS_HNL, &[]).with_ops(SIG_OPS, Arc::new(registry)),
        }
    }
}

impl Module for SigModule {
    fn name(&self) -> &str {
        "sig"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        self.module.function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        self.module.functions()
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

// ---------------------------------------------------------------------------
// Op Registry (ADR-0077 Phase 4)
// ---------------------------------------------------------------------------

/// The op names declared in `os.hnl` for the `sig` module.
pub const SIG_OPS: &[&str] = &["sig.send", "sig.list"];

/// Register the `sig` ops into the given [`OpRegistry`], dispatching to the
/// given [`Sig`] implementation.
pub fn register_sig_ops(registry: &mut OpRegistry, sig: impl Sig + 'static) {
    let sig: Arc<dyn Sig> = Arc::new(sig);
    registry.register("sig.send", Arc::new(SendFn { sig: sig.clone() }));
    registry.register("sig.list", Arc::new(ListFn { sig }));
}
