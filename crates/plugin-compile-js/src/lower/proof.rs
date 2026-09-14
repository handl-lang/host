//! Lower the proof `Term` combinators (`Rewrite`, `Path`, `Equiv`) to JS.

use handl_plugin_compile::CompileError;
use plugin_engine::{Term, term::RewriteWitness};

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

/// Lower a proof combinator to a top-level JS arrow.
pub fn lower(term: &Term) -> Result<JsAst, CompileError> {
    match term {
        Term::Rewrite {
            source,
            target,
            witness,
        } => lower_rewrite(source, target, witness),
        Term::Path {
            endpoints: (a, b),
            witness,
        } => lower_path(a, b, witness.as_deref()),
        Term::Equiv { a, b, path } => lower_equiv(a, b, path.as_deref()),
        _ => Err(CompileError::Lower(format!(
            "proof lowerer received non-proof term: {term:?}"
        ))),
    }
}

/// Lower a proof combinator applied to a concrete input expression.
pub fn lower_applied(term: &Term, input: &JsAst) -> Result<JsAst, CompileError> {
    match term {
        Term::Rewrite {
            source,
            target,
            witness,
        } => lower_rewrite_applied(source, target, witness, input),
        Term::Path {
            endpoints: (a, b),
            witness,
        } => lower_path_applied(a, b, witness.as_deref(), input),
        Term::Equiv { a, b, path } => lower_equiv_applied(a, b, path.as_deref(), input),
        _ => Err(CompileError::Lower(format!(
            "proof lowerer received non-proof term: {term:?}"
        ))),
    }
}

fn lower_rewrite(
    source: &Term,
    target: &Term,
    witness: &RewriteWitness,
) -> Result<JsAst, CompileError> {
    Ok(arrow_fn(lower_rewrite_applied(
        source,
        target,
        witness,
        &input_ident(),
    )?))
}

fn lower_rewrite_applied(
    source: &Term,
    target: &Term,
    witness: &RewriteWitness,
    input: &JsAst,
) -> Result<JsAst, CompileError> {
    let s = crate::lower::lower(source)?;
    let t = crate::lower::lower(target)?;
    let w = JsAst::Str(witness_name(witness));
    let rewrite = rt_call("rewrite", vec![s, t, w]);
    Ok(JsAst::Call {
        callee: Box::new(rewrite),
        args: vec![input.clone()],
    })
}

fn lower_path(a: &Term, b: &Term, witness: Option<&Term>) -> Result<JsAst, CompileError> {
    Ok(arrow_fn(lower_path_applied(a, b, witness, &input_ident())?))
}

fn lower_path_applied(
    a: &Term,
    b: &Term,
    witness: Option<&Term>,
    input: &JsAst,
) -> Result<JsAst, CompileError> {
    let from = crate::lower::lower(a)?;
    let to = crate::lower::lower(b)?;
    let w = match witness {
        Some(t) => crate::lower::lower(t)?,
        None => JsAst::Null,
    };
    let path = rt_call("path", vec![from, to, w]);
    Ok(JsAst::Call {
        callee: Box::new(path),
        args: vec![input.clone()],
    })
}

fn lower_equiv(a: &Term, b: &Term, path: Option<&Term>) -> Result<JsAst, CompileError> {
    Ok(arrow_fn(lower_equiv_applied(a, b, path, &input_ident())?))
}

fn lower_equiv_applied(
    a: &Term,
    b: &Term,
    path: Option<&Term>,
    input: &JsAst,
) -> Result<JsAst, CompileError> {
    let left = crate::lower::lower(a)?;
    let right = crate::lower::lower(b)?;
    let p = match path {
        Some(t) => crate::lower::lower(t)?,
        None => JsAst::Null,
    };
    let equiv = rt_call("equiv", vec![left, right, p]);
    Ok(JsAst::Call {
        callee: Box::new(equiv),
        args: vec![input.clone()],
    })
}

fn witness_name(witness: &RewriteWitness) -> String {
    match witness {
        RewriteWitness::Named(s) => s.clone(),
        RewriteWitness::Compute => "compute".to_string(),
        RewriteWitness::HandlerHint => "handler-hint".to_string(),
        RewriteWitness::Cast => "cast".to_string(),
    }
}
