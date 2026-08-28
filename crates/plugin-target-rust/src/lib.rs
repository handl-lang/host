//! Runtime plugin backed by the Rust VM interpreter.

use handl_expr::{Pattern, Stmt, StmtKind};
use handl_platform::{
    CapabilityId, CapabilityRequest, CapabilityResponse, PlatformError, Plugin, PluginContext,
};
use handl_runtime::{EvalOutput, RuntimePlugin};
use handl_vm::{Context, Error, Function, Host, VirtualMachine};
use handl_vm_backend_rust_interpreter::{
    CanonicalContext, CanonicalEval, CanonicalVm, FunctionValue, NonFunctionValue,
};
use handl_vm_common::ModuleHost;
use serde_json::Value as JsonValue;

/// A `RuntimePlugin` adapter around the canonical Rust VM.
pub struct RustVm {
    capabilities: Vec<CapabilityId>,
    modules: ModuleHost,
}

impl RustVm {
    /// Creates a new `RustVm` plugin with the built-in host and no extra libraries.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("runtime")],
            modules: ModuleHost::new(),
        }
    }

    /// Creates a new `RustVm` with the given pre-installed library `ModuleHost`.
    pub fn with_modules(modules: ModuleHost) -> Self {
        Self {
            capabilities: vec![CapabilityId::from("runtime")],
            modules,
        }
    }
}

impl Default for RustVm {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimePlugin for RustVm {
    fn name(&self) -> &str {
        "vm-rust"
    }

    fn eval(&self, stmts: &[Stmt<NonFunctionValue>]) -> Result<EvalOutput, PlatformError> {
        let host = CompositeHost {
            builtin: BuiltinHost,
            modules: self.modules.clone(),
        };
        let mut ctx = CanonicalContext::new();
        ctx.set_eval(Box::new(CanonicalEval::new(host.clone(), None, None)));
        let vm = CanonicalVm::new(host);

        let mut values = Vec::new();
        for stmt in stmts {
            match &stmt.kind {
                StmtKind::Expr(expr) => match vm.eval(expr, &mut ctx) {
                    Ok(value) => values.push(value),
                    Err(e) => return Err(PlatformError::Plugin(e.to_string())),
                },
                StmtKind::Let { pattern, value } | StmtKind::Const { pattern, value } => {
                    match vm.eval(value, &mut ctx) {
                        Ok(value) => {
                            if let Err(e) = define_simple(&mut ctx, pattern, value) {
                                return Err(PlatformError::Plugin(e.to_string()));
                            }
                        }
                        Err(e) => return Err(PlatformError::Plugin(e.to_string())),
                    }
                }
                _ => {}
            }
        }

        Ok(EvalOutput { values })
    }
}

impl Plugin for RustVm {
    fn name(&self) -> &str {
        "vm-rust"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<serde_json::Value> {
        Some(serde_json::json!({}))
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

fn define_simple(
    ctx: &mut dyn Context<NonFunctionValue>,
    pattern: &Pattern,
    value: NonFunctionValue,
) -> Result<(), Error<NonFunctionValue>> {
    match pattern {
        Pattern::Identifier(name) => {
            ctx.define(name, value);
            Ok(())
        }
        _ => Err(Error::PrimitiveError(
            "unsupported binding pattern".to_string(),
        )),
    }
}

#[derive(Clone)]
struct BuiltinHost;

impl Host<NonFunctionValue> for BuiltinHost {
    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        match name {
            "add" => Some(&ADD),
            "sub" => Some(&SUB),
            "mul" => Some(&MUL),
            "div" => Some(&DIV),
            "eq" => Some(&EQ),
            "lt" => Some(&LT),
            "and" => Some(&AND),
            "or" => Some(&OR),
            "not" | "bool.not" => Some(&NOT),
            "array.at" => Some(&ARRAY_AT),
            "obj.get" => Some(&OBJ_GET),
            "apply" => Some(&APPLY),
            "fn.pipe" => Some(&PIPE),
            _ => None,
        }
    }
}

/// A host that combines built-in functions with a `ModuleHost` of loaded libraries.
#[derive(Clone)]
struct CompositeHost {
    builtin: BuiltinHost,
    modules: ModuleHost,
}

impl Host<NonFunctionValue> for CompositeHost {
    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        self.builtin
            .function(name)
            .or_else(|| self.modules.function(name))
    }

    fn lazy_function(&self, name: &str) -> Option<&dyn handl_vm::LazyFunction<NonFunctionValue>> {
        self.modules.lazy_function(name)
    }

    fn generic_function(
        &self,
        name: &str,
    ) -> Option<&dyn handl_vm::GenericFunction<NonFunctionValue>> {
        self.modules.generic_function(name)
    }

    fn async_function(&self, name: &str) -> Option<&dyn handl_vm::AsyncFunction<NonFunctionValue>> {
        self.modules.async_function(name)
    }

    fn operation(&self, name: &str) -> Option<&dyn handl_vm::Operation<NonFunctionValue>> {
        self.modules.operation(name)
    }

    fn is_reserved(&self, name: &str) -> bool {
        self.builtin.is_reserved(name) || self.modules.is_reserved(name)
    }
}

struct Add;
struct Sub;
struct Mul;
struct Div;
struct Eq;
struct Lt;
struct And;
struct Or;
struct Not;
struct ArrayAt;
struct ObjGet;
struct Apply;
struct Pipe;

static ADD: Add = Add;
static SUB: Sub = Sub;
static MUL: Mul = Mul;
static DIV: Div = Div;
static EQ: Eq = Eq;
static LT: Lt = Lt;
static AND: And = And;
static OR: Or = Or;
static NOT: Not = Not;
static ARRAY_AT: ArrayAt = ArrayAt;
static OBJ_GET: ObjGet = ObjGet;
static APPLY: Apply = Apply;
static PIPE: Pipe = Pipe;

fn json_kind(v: &NonFunctionValue) -> &'static str {
    match v.inner() {
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
        JsonValue::String(_) => "string",
        JsonValue::Number(_) => "number",
        JsonValue::Bool(_) => "boolean",
        JsonValue::Null => "null",
    }
}

fn expect_n(args: &[NonFunctionValue], n: usize) -> Result<(), Error<NonFunctionValue>> {
    if args.len() != n {
        Err(Error::ArgumentCount {
            expected: n,
            got: args.len(),
        })
    } else {
        Ok(())
    }
}

fn as_number(v: &NonFunctionValue) -> Result<f64, Error<NonFunctionValue>> {
    v.as_number().ok_or(Error::TypeMismatch {
        expected: "number",
        got: json_kind(v),
    })
}

fn as_bool(v: &NonFunctionValue) -> Result<bool, Error<NonFunctionValue>> {
    v.as_bool().ok_or(Error::TypeMismatch {
        expected: "boolean",
        got: json_kind(v),
    })
}

impl Function<NonFunctionValue> for Add {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        expect_n(args, 2)?;
        let a = as_number(&args[0])?;
        let b = as_number(&args[1])?;
        Ok(NonFunctionValue::new(a + b))
    }
}

impl Function<NonFunctionValue> for Sub {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        expect_n(args, 2)?;
        let a = as_number(&args[0])?;
        let b = as_number(&args[1])?;
        Ok(NonFunctionValue::new(a - b))
    }
}

impl Function<NonFunctionValue> for Mul {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        expect_n(args, 2)?;
        let a = as_number(&args[0])?;
        let b = as_number(&args[1])?;
        Ok(NonFunctionValue::new(a * b))
    }
}

impl Function<NonFunctionValue> for Div {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        expect_n(args, 2)?;
        let a = as_number(&args[0])?;
        let b = as_number(&args[1])?;
        if b == 0.0 {
            return Err(Error::PrimitiveError("division by zero".to_string()));
        }
        Ok(NonFunctionValue::new(a / b))
    }
}

impl Function<NonFunctionValue> for Eq {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        expect_n(args, 2)?;
        Ok(NonFunctionValue::new(args[0].inner() == args[1].inner()))
    }
}

impl Function<NonFunctionValue> for Lt {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        expect_n(args, 2)?;
        let a = as_number(&args[0])?;
        let b = as_number(&args[1])?;
        Ok(NonFunctionValue::new(a < b))
    }
}

impl Function<NonFunctionValue> for And {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        expect_n(args, 2)?;
        let a = as_bool(&args[0])?;
        let b = as_bool(&args[1])?;
        Ok(NonFunctionValue::new(a && b))
    }
}

impl Function<NonFunctionValue> for Or {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        expect_n(args, 2)?;
        let a = as_bool(&args[0])?;
        let b = as_bool(&args[1])?;
        Ok(NonFunctionValue::new(a || b))
    }
}

impl Function<NonFunctionValue> for Not {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        expect_n(args, 1)?;
        let a = as_bool(&args[0])?;
        Ok(NonFunctionValue::new(!a))
    }
}

impl Function<NonFunctionValue> for ArrayAt {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        expect_n(args, 2)?;
        let arr = args[0].as_array().ok_or(Error::TypeMismatch {
            expected: "array",
            got: json_kind(&args[0]),
        })?;
        let idx = as_number(&args[1])? as usize;
        Ok(NonFunctionValue::new(
            arr.get(idx).cloned().unwrap_or(JsonValue::Null),
        ))
    }
}

impl Function<NonFunctionValue> for ObjGet {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
        expect_n(args, 2)?;
        let obj = args[0].as_object().ok_or(Error::TypeMismatch {
            expected: "object",
            got: json_kind(&args[0]),
        })?;
        let key = args[1].as_str().ok_or(Error::TypeMismatch {
            expected: "string",
            got: json_kind(&args[1]),
        })?;
        Ok(NonFunctionValue::new(
            obj.get(key).cloned().unwrap_or(JsonValue::Null),
        ))
    }
}

impl Function<NonFunctionValue> for Apply {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
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
}

impl Function<NonFunctionValue> for Pipe {
    fn call(
        &self,
        args: &[NonFunctionValue],
        ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, Error<NonFunctionValue>> {
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
}
