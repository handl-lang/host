//! handl-vm-backend-rust-interpreter
//!
//! Stub implementation of the canonical tree-walking virtual machine.
//! All re-exports and public types are preserved so workspace consumers
//! continue to compile; the actual evaluation and handler methods return
//! `Error::PrimitiveError` as a placeholder.

pub use handl_vm::{
    AsyncEval, AsyncFunction, Context, Error, Eval, Expr, Function, GenericFunction, HandlerClause,
    Host, HostRef, LazyFunction, NoopHost, Operation, OperationSig, Pattern, Value, VirtualMachine,
};
pub use handl_vm_values_json::{CanonicalContext, FunctionValue, NonFunctionValue};

use std::future::{ready, Future};
use std::pin::Pin;

// Canonical virtual machine, generic over the concrete host.
pub struct CanonicalVm<H: Host<NonFunctionValue> = NoopHost> {
    host: H,
}

impl<H: Host<NonFunctionValue>> CanonicalVm<H> {
    pub fn new(host: H) -> Self {
        Self { host }
    }

    pub fn host(&self) -> &H {
        &self.host
    }
}

impl<H: Host<NonFunctionValue>> VirtualMachine<NonFunctionValue> for CanonicalVm<H> {
    fn eval(
        &self,
        _expr: &Expr<NonFunctionValue>,
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        Err(Error::PrimitiveError(
            "CanonicalVm::eval not implemented".to_string(),
        ))
    }

    fn eval_async(
        &self,
        _expr: &Expr<NonFunctionValue>,
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> impl Future<Output = Result<NonFunctionValue, Error<NonFunctionValue>>> {
        ready(Err(Error::PrimitiveError(
            "CanonicalVm::eval_async not implemented".to_string(),
        )))
    }
}

pub use CanonicalVm as Vm;

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

// Serializable continuation scaffolding.
#[derive(Debug, Clone, PartialEq)]
pub struct Continuation {
    pub frames: Vec<Frame>,
}

impl Continuation {
    pub fn empty() -> Self {
        Self { frames: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::Value::Array(Vec::new())
    }

    pub fn from_json(_value: &serde_json::Value) -> Result<Self, Error<NonFunctionValue>> {
        Ok(Self::empty())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub name: String,
    pub evaluated: Vec<NonFunctionValue>,
    pub pending: Vec<Expr<NonFunctionValue>>,
    pub handlers: Vec<HandlerClause>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EvalResult<V: Value> {
    Done(V),
    Yielded { value: V, continuation: Continuation },
    Error(Error<V>),
}

// Canonical constructors.
pub fn canonical_vm() -> CanonicalVm {
    CanonicalVm::new(NoopHost)
}

pub fn canonical_eval<H: Host<NonFunctionValue>>(
    host: H,
    max_depth: Option<usize>,
    max_calls: Option<usize>,
) -> CanonicalEval<H> {
    CanonicalEval::new(host, max_depth, max_calls)
}

// Stub `Function` handlers.
#[derive(Debug, Clone, Copy, Default)]
pub struct ArrayHandler;

impl Function<NonFunctionValue> for ArrayHandler {
    fn call(
        &self,
        _args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        Err(Error::PrimitiveError(
            "ArrayHandler not implemented".to_string(),
        ))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CallHandler;

impl Function<NonFunctionValue> for CallHandler {
    fn call(
        &self,
        _args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        Err(Error::PrimitiveError(
            "CallHandler not implemented".to_string(),
        ))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct HandleHandler;

impl Function<NonFunctionValue> for HandleHandler {
    fn call(
        &self,
        _args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        Err(Error::PrimitiveError(
            "HandleHandler not implemented".to_string(),
        ))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ObjectHandler;

impl Function<NonFunctionValue> for ObjectHandler {
    fn call(
        &self,
        _args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        Err(Error::PrimitiveError(
            "ObjectHandler not implemented".to_string(),
        ))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ValueHandler;

impl Function<NonFunctionValue> for ValueHandler {
    fn call(
        &self,
        _args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        Err(Error::PrimitiveError(
            "ValueHandler not implemented".to_string(),
        ))
    }
}
