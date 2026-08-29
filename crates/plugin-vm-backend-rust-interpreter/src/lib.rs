//! handl-plugin-vm-backend-rust-interpreter
//!
//! Canonical tree-walking virtual machine implementation.

pub mod continuation;
pub mod handlers;
pub mod pattern;
pub mod vm;

pub use handl_plugin_vm::{
    AsyncEval, AsyncFunction, Context, Error, Eval, Expr, Function, GenericFunction, HandlerClause,
    CallableLookup, CallableLookupRef, LazyFunction, NoopHost, Operation, OperationSig, Pattern, Value, VirtualMachine,
};
pub use handl_plugin_vm_values_json::{CanonicalContext, FunctionValue, NonFunctionValue};

pub use vm::CanonicalVm;

/// Type alias for backward compatibility.
pub type Vm<H = NoopHost> = CanonicalVm<H>;

pub use continuation::{Continuation, EvalResult, Frame};
pub use handlers::{
    ApplyHandler, ArrayHandler, AssignHandler, CallHandler, FieldAccessHandler, FunctionHandler,
    HandleHandler, IndexHandler, ObjectHandler, ValueHandler, VarHandler,
};

use std::future::{Future, ready};
use std::pin::Pin;

// Canonical `Eval` / `AsyncEval` callback.
#[allow(dead_code)]
pub struct CanonicalEval<H> {
    host: H,
    max_depth: Option<usize>,
    max_calls: Option<usize>,
}

impl<H> CanonicalEval<H> {
    pub fn new(host: H, max_depth: Option<usize>, max_calls: Option<usize>) -> Self {
        Self {
            host,
            max_depth,
            max_calls,
        }
    }
}

impl<H: CallableLookup<NonFunctionValue> + Clone + 'static> Eval<NonFunctionValue> for CanonicalEval<H> {
    fn eval_expr(
        &self,
        expr: &str,
        vars: &[(String, NonFunctionValue)],
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        let json: serde_json::Value = serde_json::from_str(expr).map_err(|e| {
            Error::PrimitiveError(format!("failed to parse expression JSON: {e}"))
        })?;
        let parsed = continuation::expr_from_json(&json)?;

        let vm = CanonicalVm::new(self.host.clone());
        let mut ctx = CanonicalContext::new();
        if let Some(max) = self.max_depth {
            ctx.set_max_depth(max);
        }
        if let Some(max) = self.max_calls {
            ctx.set_max_calls(max);
        }
        ctx.set_eval(Box::new(CanonicalEval::new(
            self.host.clone(),
            self.max_depth,
            self.max_calls,
        )));
        for (name, value) in vars {
            ctx.define(name.clone(), value.clone());
        }
        vm.eval(&parsed, &mut ctx)
    }
}

impl<H: CallableLookup<NonFunctionValue>> AsyncEval<NonFunctionValue> for CanonicalEval<H> {
    fn eval_expr_async<'a>(
        &'a self,
        _expr: &'a str,
        _vars: &'a [(String, NonFunctionValue)],
    ) -> Pin<Box<dyn Future<Output = Result<NonFunctionValue, Error<NonFunctionValue>>> + 'a>> {
        Box::pin(ready(Err(Error::PrimitiveError(
            "CanonicalEval::eval_expr_async not implemented".to_string(),
        ))))
    }
}

// Canonical constructors.
pub fn canonical_vm() -> Vm {
    Vm::new(NoopHost)
}

pub fn canonical_eval<H: CallableLookup<NonFunctionValue>>(
    host: H,
    max_depth: Option<usize>,
    max_calls: Option<usize>,
) -> CanonicalEval<H> {
    CanonicalEval::new(host, max_depth, max_calls)
}

#[cfg(test)]
mod vm_tests;

#[cfg(test)]
mod continuation_tests;

#[cfg(test)]
mod handler_tests;

#[cfg(test)]
mod operation_tests;

#[cfg(test)]
mod generic_tests;
