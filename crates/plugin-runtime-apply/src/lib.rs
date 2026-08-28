//! Apply / pipe runtime builtins: `apply`, `fn.pipe`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use handl_platform::{
    expect_n, json_kind, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    ContributionContext, PlatformError, Plugin, PluginContext, RuntimeBehavior, RuntimeHost,
    RuntimeRule,
};
use handl_vm::{Context, Error};
use handl_vm_values_json::{FunctionValue, NonFunctionValue};

/// Apply / pipe built-in family.
pub struct ApplyFamily;

impl RuntimeBehavior for ApplyFamily {
    fn kind(&self) -> &str {
        "runtime.apply"
    }

    fn provides(&self) -> &[&'static str] {
        &["apply", "fn.pipe"]
    }

    fn call(
        &self,
        name: &str,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        match name {
            "apply" => {
                if args.is_empty() {
                    return Err(Error::ArgumentCount {
                        expected: 1,
                        got: 0,
                    });
                }
                let fv = FunctionValue::from_json(args[0].inner()).ok_or(Error::TypeMismatch {
                    expected: "function",
                    got: json_kind(&args[0]),
                })?;
                let call_args = &args[1..];
                if call_args.len() != fv.params.len() {
                    return Err(Error::ArgumentCount {
                        expected: fv.params.len(),
                        got: call_args.len(),
                    });
                }
                let vars: Vec<(String, NonFunctionValue)> = fv
                    .params
                    .iter()
                    .zip(call_args.iter())
                    .map(|((name, _), value)| (name.clone(), value.clone()))
                    .collect();
                ctx.eval_expr(&fv.body, &vars)
            }
            "fn.pipe" => {
                expect_n(args, 2)?;
                let left = &args[0];
                let fv = FunctionValue::from_json(args[1].inner()).ok_or(Error::TypeMismatch {
                    expected: "function",
                    got: json_kind(&args[1]),
                })?;
                if fv.params.len() != 1 {
                    return Err(Error::ArgumentCount {
                        expected: 1,
                        got: fv.params.len(),
                    });
                }
                let name = fv.params[0].0.clone();
                ctx.eval_expr(&fv.body, &[(name, left.clone())])
            }
            _ => Err(Error::PrimitiveError(format!(
                "unknown apply builtin: {name}"
            ))),
        }
    }
}

/// Registers the apply/pipe family with the platform `RuntimeHost`.
pub struct ApplyPlugin {
    registered: AtomicBool,
}

impl ApplyPlugin {
    /// Create a new `ApplyPlugin`.
    pub fn new() -> Self {
        Self {
            registered: AtomicBool::new(false),
        }
    }
}

impl Default for ApplyPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ApplyPlugin {
    fn id(&self) -> &str {
        "runtime.apply"
    }

    fn name(&self) -> &str {
        "apply"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        if !self.registered.load(Ordering::SeqCst) {
            self.registered.store(true, Ordering::SeqCst);
            ctx.contribute::<RuntimeHost>(RuntimeRule::new(Arc::new(ApplyFamily)))?;
        }
        Ok(())
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}
