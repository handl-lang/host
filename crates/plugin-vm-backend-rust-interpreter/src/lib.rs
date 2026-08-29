//! handl-plugin-vm-backend-rust-interpreter
//!
//! Canonical tree-walking virtual machine implementation.

pub mod continuation;
pub mod handlers;
pub mod pattern;
pub mod vm;

pub use handl_plugin_vm::{
    AsyncEval, AsyncFunction, Context, Error, Eval, Expr, Function, GenericFunction, HandlerClause,
    Host, HostRef, LazyFunction, NoopHost, Operation, OperationSig, Pattern, Value, VirtualMachine,
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

impl<H: Host<NonFunctionValue>> Eval<NonFunctionValue> for CanonicalEval<H> {
    fn eval_expr(
        &self,
        _expr: &str,
        _vars: &[(String, NonFunctionValue)],
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        Err(Error::PrimitiveError(
            "CanonicalEval::eval_expr not implemented".to_string(),
        ))
    }
}

impl<H: Host<NonFunctionValue>> AsyncEval<NonFunctionValue> for CanonicalEval<H> {
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

pub fn canonical_eval<H: Host<NonFunctionValue>>(
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
