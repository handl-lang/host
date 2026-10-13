//! Lower the proof `Term` combinators to JS.
//!
//! Only `Rewrite` reaches this module: it is the only proof shape in the
//! closed `Term` vocabulary. `Path` / `Equiv` (with `Refine`, `CellId`,
//! `JElim`, `Whisker`, …) are *derived* operators that ADR-0119 §7 folded into
//! `Term::Opaque`, so they arrive as opaque ADR-0103 records at the appropriate
//! dimension rather than as variants — there is nothing left to lower here.
//! Dispatched from `lower::lower` / `lower::lower_applied`.

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

fn witness_name(witness: &RewriteWitness) -> String {
    match witness {
        RewriteWitness::Named(s) => s.clone(),
        RewriteWitness::Compute => "compute".to_string(),
        RewriteWitness::HandlerHint => "handler-hint".to_string(),
        RewriteWitness::Cast => "cast".to_string(),
    }
}
