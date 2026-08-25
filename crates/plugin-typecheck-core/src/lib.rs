//! Typecheck plugin backed by a simple canonical expression typechecker.

use std::collections::HashMap;

use handl_ast_canonical::{Expr, Pattern};
use handl_expr::{Stmt, StmtKind};
use handl_platform::{
    CapabilityId, CapabilityRequest, CapabilityResponse, PlatformError, Plugin, PluginContext,
};
use handl_runtime::{TypecheckedOutput, TypecheckPlugin};
use handl_vm_values_json::NonFunctionValue;
use serde_json::Value as JsonValue;

/// A `TypecheckPlugin` adapter for the canonical expression language.
pub struct TypecheckCore {
    capabilities: Vec<CapabilityId>,
}

impl TypecheckCore {
    /// Creates a new `TypecheckCore` plugin.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("typecheck")],
        }
    }
}

impl Default for TypecheckCore {
    fn default() -> Self {
        Self::new()
    }
}

impl TypecheckPlugin for TypecheckCore {
    fn name(&self) -> &str {
        "typecheck-core"
    }

    fn typecheck(&self, stmts: &[Stmt]) -> Result<TypecheckedOutput, PlatformError> {
        let mut env = HashMap::new();
        let mut errors = Vec::new();

        for stmt in stmts {
            match &stmt.kind {
                StmtKind::Let { pattern, value } | StmtKind::Const { pattern, value } => {
                    let ty = infer_expr(value, &mut env, &mut errors);
                    bind_pattern(pattern, ty, &mut env);
                }
                StmtKind::Expr(expr) => {
                    infer_expr(expr, &mut env, &mut errors);
                }
                _ => {}
            }
        }

        Ok(TypecheckedOutput {
            success: errors.is_empty(),
        })
    }
}

impl Plugin for TypecheckCore {
    fn name(&self) -> &str {
        "typecheck-core"
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

#[derive(Debug, Clone, PartialEq)]
enum Ty {
    Number,
    String,
    Boolean,
    Null,
    Array(Box<Ty>),
    Object(HashMap<String, Ty>),
    Function(Vec<Ty>, Box<Ty>),
    Unknown,
}

fn value_to_ty(v: &NonFunctionValue) -> Ty {
    match v.inner() {
        JsonValue::Number(_) => Ty::Number,
        JsonValue::String(_) => Ty::String,
        JsonValue::Bool(_) => Ty::Boolean,
        JsonValue::Null => Ty::Null,
        JsonValue::Array(_) => Ty::Array(Box::new(Ty::Unknown)),
        JsonValue::Object(map) => {
            let mut props = HashMap::new();
            for (k, inner) in map {
                props.insert(k.clone(), NonFunctionValue::new(inner.clone()).into());
            }
            Ty::Object(props)
        }
    }
}

impl From<NonFunctionValue> for Ty {
    fn from(v: NonFunctionValue) -> Self {
        value_to_ty(&v)
    }
}

fn bind_pattern(pattern: &Pattern, ty: Ty, env: &mut HashMap<String, Ty>) {
    if let Pattern::Identifier(name) = pattern {
        env.insert(name.clone(), ty);
    }
}

fn compatible(actual: &Ty, expected: &Ty) -> bool {
    if expected == &Ty::Unknown || actual == &Ty::Unknown {
        return true;
    }
    actual == expected
}

fn check_call(params: &[Ty], args: &[Ty], name: &str, errors: &mut Vec<String>) {
    if params.len() != args.len() {
        errors.push(format!(
            "{}: expected {} arguments, got {}",
            name,
            params.len(),
            args.len()
        ));
        return;
    }
    for (i, (p, a)) in params.iter().zip(args.iter()).enumerate() {
        if !compatible(a, p) {
            errors.push(format!(
                "{}: argument {} expected {}, got {}",
                name,
                i,
                fmt(p),
                fmt(a)
            ));
        }
    }
}

fn fmt(ty: &Ty) -> String {
    match ty {
        Ty::Number => "number".to_string(),
        Ty::String => "string".to_string(),
        Ty::Boolean => "boolean".to_string(),
        Ty::Null => "null".to_string(),
        Ty::Array(_) => "array".to_string(),
        Ty::Object(_) => "object".to_string(),
        Ty::Function(_, _) => "function".to_string(),
        Ty::Unknown => "unknown".to_string(),
    }
}

fn builtin_sig(name: &str) -> Option<(Vec<Ty>, Ty)> {
    match name {
        "add" | "sub" | "mul" | "div" => Some((vec![Ty::Number, Ty::Number], Ty::Number)),
        "eq" | "lt" => Some((vec![Ty::Number, Ty::Number], Ty::Boolean)),
        "and" | "or" => Some((vec![Ty::Boolean, Ty::Boolean], Ty::Boolean)),
        "not" | "bool.not" => Some((vec![Ty::Boolean], Ty::Boolean)),
        "array.at" => Some((vec![Ty::Array(Box::new(Ty::Unknown)), Ty::Number], Ty::Unknown)),
        "obj.get" => Some((vec![Ty::Object(HashMap::new()), Ty::String], Ty::Unknown)),
        "apply" | "fn.pipe" => Some((vec![Ty::Unknown, Ty::Unknown], Ty::Unknown)),
        _ => None,
    }
}

fn infer_expr(expr: &Expr<NonFunctionValue>, env: &mut HashMap<String, Ty>, errors: &mut Vec<String>) -> Ty {
    match expr {
        Expr::Value(v) => value_to_ty(v),

        Expr::Var(name) => env.get(name).cloned().unwrap_or(Ty::Unknown),

        Expr::Call { name, args, type_args: _ } => {
            let arg_tys: Vec<Ty> = args
                .iter()
                .map(|a| infer_expr(a, env, errors))
                .collect();

            if let Some(fn_ty) = env.get(name) {
                match fn_ty {
                    Ty::Function(params, ret) => {
                        check_call(params, &arg_tys, name, errors);
                        return *ret.clone();
                    }
                    _ => {
                        errors.push(format!("{} is not a function", name));
                        return Ty::Unknown;
                    }
                }
            }

            if let Some((params, ret)) = builtin_sig(name) {
                check_call(&params, &arg_tys, name, errors);
                return ret;
            }

            errors.push(format!("undefined function: {}", name));
            Ty::Unknown
        }

        Expr::Apply { target, args } => {
            let arg_tys: Vec<Ty> = args
                .iter()
                .map(|a| infer_expr(a, env, errors))
                .collect();
            let target_ty = infer_expr(target, env, errors);
            match target_ty {
                Ty::Function(params, ret) => {
                    check_call(&params, &arg_tys, "<apply>", errors);
                    *ret
                }
                _ => {
                    errors.push("target of apply is not a function".to_string());
                    Ty::Unknown
                }
            }
        }

        Expr::Function { params, .. } => {
            let ptypes = params.iter().map(|_| Ty::Unknown).collect();
            Ty::Function(ptypes, Box::new(Ty::Unknown))
        }

        Expr::Assign { target, value } => {
            let ty = infer_expr(value, env, errors);
            bind_pattern(target, ty.clone(), env);
            ty
        }

        Expr::Array(elements) => {
            for e in elements {
                infer_expr(e, env, errors);
            }
            Ty::Array(Box::new(Ty::Unknown))
        }

        Expr::Object(entries) => {
            let mut props = HashMap::new();
            for (k, v) in entries {
                let t = infer_expr(v, env, errors);
                props.insert(k.clone(), t);
            }
            Ty::Object(props)
        }

        Expr::Index { target, index } => {
            let t = infer_expr(target, env, errors);
            infer_expr(index, env, errors);
            match t {
                Ty::Array(item) => *item,
                _ => Ty::Unknown,
            }
        }

        Expr::FieldAccess { target, field } => {
            let t = infer_expr(target, env, errors);
            match t {
                Ty::Object(props) => props.get(field).cloned().unwrap_or(Ty::Unknown),
                _ => Ty::Unknown,
            }
        }

        Expr::Handle { body, .. } => infer_expr(body, env, errors),
    }
}
