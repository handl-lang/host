//! Lower the higher-order `Term` combinators (`Child`, `Handle`) to JS.

use handl_plugin_compile::CompileError;
use plugin_engine::Term;

use crate::JsAst;

const INPUT_NAME: &str = "__in";

fn input_ident() -> JsAst {
    JsAst::Ident(INPUT_NAME.to_string())
}

fn rt_call(name: &str, args: Vec<JsAst>) -> JsAst {
    JsAst::Call {
        callee: Box::new(JsAst::Member {
            obj: Box::new(JsAst::Ident("__handl_rt".to_string())),
            prop: name.to_string(),
        }),
        args,
    }
}

fn arrow_fn(body: JsAst) -> JsAst {
    JsAst::ArrowFn {
        params: vec![INPUT_NAME.to_string()],
        body: Box::new(body),
    }
}

/// Lower a higher-order combinator to a top-level JS arrow.
pub fn lower(term: &Term) -> Result<JsAst, CompileError> {
    match term {
        Term::Child { role } => lower_child(role),
        Term::Handle {
            body,
            target,
            impl_,
        } => lower_handle(body, target, impl_),
        _ => Err(CompileError::Lower(format!(
            "higher lowerer received non-higher term: {term:?}"
        ))),
    }
}

/// Lower a higher-order combinator applied to a concrete input expression.
pub fn lower_applied(term: &Term, input: &JsAst) -> Result<JsAst, CompileError> {
    match term {
        Term::Child { role } => Ok(lower_child_applied(role, input)),
        Term::Handle {
            body,
            target,
            impl_,
        } => lower_handle_applied(body, target, impl_, input),
        _ => Err(CompileError::Lower(format!(
            "higher lowerer received non-higher term: {term:?}"
        ))),
    }
}

fn lower_child(role: &str) -> Result<JsAst, CompileError> {
    Ok(arrow_fn(lower_child_applied(role, &input_ident())))
}

fn lower_child_applied(role: &str, input: &JsAst) -> JsAst {
    rt_call("child", vec![input.clone(), JsAst::Str(role.to_string())])
}

fn lower_handle(body: &Term, target: &str, impl_: &Term) -> Result<JsAst, CompileError> {
    Ok(arrow_fn(lower_handle_applied(
        body,
        target,
        impl_,
        &input_ident(),
    )?))
}

fn lower_handle_applied(
    body: &Term,
    target: &str,
    impl_: &Term,
    input: &JsAst,
) -> Result<JsAst, CompileError> {
    let body_fn = crate::lower::lower(body)?;
    let impl_fn = crate::lower::lower(impl_)?;
    let handle = rt_call(
        "handle",
        vec![body_fn, JsAst::Str(target.to_string()), impl_fn],
    );
    Ok(JsAst::Call {
        callee: Box::new(handle),
        args: vec![input.clone()],
    })
}
