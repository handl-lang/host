//! Lower the primitive `Term` combinators (`Arr`, `Const`, `Op`, `Seq`) to JS.

use handl_plugin_compile::CompileError;
use plugin_engine::{ArrSource, Term};
use serde_json::Value;

use super::allow_inlining;
use super::lower_applied as lower_sub;
use super::{JsAst, JsBinOp, lower_value};

const INPUT_NAME: &str = "__in";

/// Lower one of the four primitive terms to a JS expression.
pub fn lower(term: &Term) -> Result<JsAst, CompileError> {
    match term {
        Term::Arr { value, .. } => {
            let body = lower_value(value)?;
            Ok(JsAst::ArrowFn {
                params: vec![INPUT_NAME.to_string()],
                body: Box::new(body),
            })
        }
        Term::Const { value } => lower_value(value),
        Term::Seq { axis, fst, snd } => {
            check_axis_zero(axis)?;
            let param = JsAst::Ident(INPUT_NAME.to_string());
            let body = lower_sub(snd, &lower_sub(fst, &param)?)?;
            Ok(JsAst::ArrowFn {
                params: vec![INPUT_NAME.to_string()],
                body: Box::new(body),
            })
        }
        Term::Op {
            name,
            args,
            source,
            lit,
            ..
        } => lower_op(name, args, source.clone(), lit.as_ref()),
        _ => Err(CompileError::Lower(format!(
            "primitive lowerer received non-primitive term: {term:?}"
        ))),
    }
}

/// Lower the application of a primitive term to an input value.
pub fn lower_applied(term: &Term, _input: &JsAst) -> Result<JsAst, CompileError> {
    match term {
        Term::Arr { value, .. } | Term::Const { value } => lower_value(value),
        Term::Op { .. } => Ok(JsAst::Call {
            callee: Box::new(lower(term)?),
            args: vec![_input.clone()],
        }),
        Term::Seq { axis, fst, snd } => {
            check_axis_zero(axis)?;
            lower_sub(snd, &lower_sub(fst, _input)?)
        }
        _ => Err(CompileError::Lower(format!(
            "primitive lowerer received non-primitive term: {term:?}"
        ))),
    }
}

/// ADR-0120 Phase 2: `>>>` is `>>>[0]`. The JS backend only implements
/// axis-0 composition; non-zero or non-literal axes are structurally valid
/// but rejected here pending the multi-axis execution increment.
fn check_axis_zero(axis: &Term) -> Result<(), CompileError> {
    match axis.nat_literal() {
        Some(0) => Ok(()),
        Some(k) => Err(CompileError::Lower(format!(
            "Seq: axis-{k} composition is not yet implemented (ADR-0120 Phase 2)"
        ))),
        None => Err(CompileError::Lower(
            "Seq: non-literal composition axis is not yet supported (ADR-0120 Phase 2)".to_string(),
        )),
    }
}

fn lower_op(
    name: &str,
    args: &[Term],
    source: ArrSource,
    lit: Option<&Value>,
) -> Result<JsAst, CompileError> {
    let param = JsAst::Ident(INPUT_NAME.to_string());

    if let ArrSource::Foreign(module) = &source {
        return lower_foreign_op(name, args, module, lit);
    }

    let binop = |op: JsBinOp| -> Result<JsAst, CompileError> {
        if args.len() != 2 {
            return Err(CompileError::InvalidInput(format!(
                "binary op `{name}` requires 2 arguments, got {}",
                args.len()
            )));
        }
        let left = lower_sub(&args[0], &param)?;
        let right = lower_sub(&args[1], &param)?;
        Ok(JsAst::BinOp {
            op,
            left: Box::new(left),
            right: Box::new(right),
        })
    };

    let body = match name {
        "add" => binop(JsBinOp::Add)?,
        "sub" => binop(JsBinOp::Sub)?,
        "mul" => binop(JsBinOp::Mul)?,
        "div" => binop(JsBinOp::Div)?,
        "concat" => {
            if args.len() != 2 {
                return Err(CompileError::InvalidInput(format!(
                    "concat requires 2 arguments, got {}",
                    args.len()
                )));
            }
            let left = lower_sub(&args[0], &param)?;
            let right = lower_sub(&args[1], &param)?;
            JsAst::Call {
                callee: Box::new(JsAst::Member {
                    obj: Box::new(JsAst::Ident("__handl_rt".to_string())),
                    prop: "concat".to_string(),
                }),
                args: vec![left, right],
            }
        }
        "proj_env" => {
            let env = if args.is_empty() {
                param
            } else {
                lower_sub(&args[0], &param)?
            };
            let field = lit.and_then(|v| v.as_str()).ok_or_else(|| {
                CompileError::InvalidInput("proj_env missing lit name".to_string())
            })?;
            JsAst::Call {
                callee: Box::new(JsAst::Member {
                    obj: Box::new(JsAst::Ident("__handl_rt".to_string())),
                    prop: "projEnv".to_string(),
                }),
                args: vec![env, JsAst::Str(field.to_string())],
            }
        }
        _ => {
            let lowered_args: Vec<JsAst> = args
                .iter()
                .map(|a| lower_sub(a, &param))
                .collect::<Result<_, _>>()?;
            JsAst::Call {
                callee: Box::new(JsAst::Member {
                    obj: Box::new(JsAst::Ident("__handl_rt".to_string())),
                    prop: name.to_string(),
                }),
                args: lowered_args,
            }
        }
    };

    Ok(JsAst::ArrowFn {
        params: vec![INPUT_NAME.to_string()],
        body: Box::new(body),
    })
}

fn lower_foreign_op(
    name: &str,
    args: &[Term],
    module: &str,
    lit: Option<&Value>,
) -> Result<JsAst, CompileError> {
    let param = JsAst::Ident(INPUT_NAME.to_string());
    let lowered_args: Vec<JsAst> = args
        .iter()
        .map(|a| lower_sub(a, &param))
        .collect::<Result<_, _>>()?;

    if !allow_inlining() {
        return emit_js_call(module, name, &lowered_args, lit);
    }

    if let Some(lit) = lit {
        let desc = crate::security::validate_foreign_descriptor(lit)?;
        if crate::security::is_safe_for_inlining(&desc) {
            return Ok(inline_foreign_call(module, name, lowered_args));
        }
    }

    emit_js_call(module, name, &lowered_args, lit)
}

fn inline_foreign_call(module: &str, name: &str, args: Vec<JsAst>) -> JsAst {
    JsAst::Call {
        callee: Box::new(JsAst::Index {
            obj: Box::new(JsAst::Index {
                obj: Box::new(JsAst::Ident("globalThis".to_string())),
                key: Box::new(JsAst::Str(module.to_string())),
            }),
            key: Box::new(JsAst::Str(name.to_string())),
        }),
        args,
    }
}

fn emit_js_call(
    module: &str,
    name: &str,
    args: &[JsAst],
    maybe_desc: Option<&Value>,
) -> Result<JsAst, CompileError> {
    let descriptors = match maybe_desc {
        Some(v) => crate::security::validate_foreign_descriptor(v)?.to_json(),
        None => Value::Null,
    };
    let raw = serde_json::to_string(&descriptors).map_err(|e| CompileError::Emit(e.to_string()))?;
    Ok(JsAst::Call {
        callee: Box::new(JsAst::Member {
            obj: Box::new(JsAst::Ident("__handl_rt".to_string())),
            prop: "jsCall".to_string(),
        }),
        args: vec![
            JsAst::Str(module.to_string()),
            JsAst::Str(name.to_string()),
            JsAst::Array(args.to_vec()),
            JsAst::Raw(raw),
        ],
    })
}
