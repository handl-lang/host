//! Lower the control-flow `Term` combinators (`First`, `Choice`, `Loop`, `Iter`, `App`) to JS.

use handl_plugin_compile::CompileError;
use plugin_engine::Term;
use serde_json::Number;

use crate::lower::{JsAst, JsBinOp, JsUnaryOp};

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

fn let_decl(name: &str, value: JsAst) -> JsAst {
    JsAst::Let {
        name: name.to_string(),
        value: Box::new(value),
    }
}

fn assign(name: &str, value: JsAst) -> JsAst {
    JsAst::Assign {
        name: name.to_string(),
        value: Box::new(value),
    }
}

/// Lower a control combinator to a top-level JS arrow.
pub fn lower(term: &Term) -> Result<JsAst, CompileError> {
    match term {
        Term::First { inner } => lower_first(inner),
        Term::Choice { l, r } => lower_choice(l, r),
        Term::Loop { body } => lower_loop(body),
        Term::Iter {
            fuel,
            predicate,
            step,
            initial,
        } => lower_iter(fuel, predicate, step, initial),
        Term::App { callee, args } => lower_app(callee, args),
        _ => Err(CompileError::Lower(format!(
            "control lowerer received non-control term: {term:?}"
        ))),
    }
}

/// Lower a control combinator applied to a concrete input expression.
pub fn lower_applied(term: &Term, input: &JsAst) -> Result<JsAst, CompileError> {
    match term {
        Term::First { inner } => lower_first_applied(inner, input),
        Term::Choice { l, r } => lower_choice_applied(l, r, input),
        Term::Loop { body } => lower_loop_applied(body, input),
        Term::Iter {
            fuel,
            predicate,
            step,
            initial,
        } => lower_iter_applied(fuel, predicate, step, initial, input),
        Term::App { callee, args } => lower_app_applied(callee, args, input),
        _ => Err(CompileError::Lower(format!(
            "control lowerer received non-control term: {term:?}"
        ))),
    }
}

/// `First { inner }` — run `inner` on the first component of a pair.
pub fn lower_first(inner: &Term) -> Result<JsAst, CompileError> {
    let param = input_ident();
    Ok(arrow_fn(lower_first_applied(inner, &param)?))
}

fn lower_first_applied(inner: &Term, input: &JsAst) -> Result<JsAst, CompileError> {
    let fst = rt_call("fst", vec![input.clone()]);
    let snd = rt_call("snd", vec![input.clone()]);
    let inner_out = super::lower_applied(inner, &fst)?;
    Ok(rt_call("mkPair", vec![inner_out, snd]))
}

/// `Choice { l, r }` — route on `Either`.
pub fn lower_choice(l: &Term, r: &Term) -> Result<JsAst, CompileError> {
    let param = input_ident();
    Ok(arrow_fn(lower_choice_applied(l, r, &param)?))
}

fn lower_choice_applied(l: &Term, r: &Term, input: &JsAst) -> Result<JsAst, CompileError> {
    let cond = rt_call("isLeft", vec![input.clone()]);
    let left_val = rt_call("leftValue", vec![input.clone()]);
    let right_val = rt_call("rightValue", vec![input.clone()]);
    let left = super::lower_applied(l, &left_val)?;
    let right = super::lower_applied(r, &right_val)?;
    let iife = JsAst::ArrowFn {
        params: vec![],
        body: Box::new(JsAst::Block(vec![JsAst::If {
            cond: Box::new(cond),
            then_branch: Box::new(JsAst::Block(vec![JsAst::Return(Box::new(left))])),
            else_branch: Some(Box::new(JsAst::Block(vec![JsAst::Return(Box::new(right))]))),
        }])),
    };
    Ok(JsAst::Call {
        callee: Box::new(iife),
        args: vec![],
    })
}

/// `Loop { body }` — bounded feedback loop.
pub fn lower_loop(body: &Term) -> Result<JsAst, CompileError> {
    let param = input_ident();
    Ok(arrow_fn(lower_loop_applied(body, &param)?))
}

fn lower_loop_applied(body: &Term, input: &JsAst) -> Result<JsAst, CompileError> {
    let body_fn = super::lower(body)?;
    let saved = "__saved";
    let state = "__state";
    let cnt = "__cnt";
    let pair = "__pair";
    let out = "__out";
    let max_iterations = JsAst::Num(Number::from(100_000));

    let block = JsAst::Block(vec![
        let_decl(saved, input.clone()),
        let_decl(state, JsAst::Null),
        let_decl(cnt, JsAst::Num(Number::from(0))),
        JsAst::While {
            cond: Box::new(JsAst::BinOp {
                op: JsBinOp::Lt,
                left: Box::new(JsAst::Ident(cnt.to_string())),
                right: Box::new(max_iterations),
            }),
            body: Box::new(JsAst::Block(vec![
                assign(
                    cnt,
                    JsAst::BinOp {
                        op: JsBinOp::Add,
                        left: Box::new(JsAst::Ident(cnt.to_string())),
                        right: Box::new(JsAst::Num(Number::from(1))),
                    },
                ),
                let_decl(
                    pair,
                    rt_call(
                        "mkPair",
                        vec![
                            JsAst::Ident(saved.to_string()),
                            JsAst::Ident(state.to_string()),
                        ],
                    ),
                ),
                let_decl(
                    out,
                    JsAst::Call {
                        callee: Box::new(body_fn),
                        args: vec![JsAst::Ident(pair.to_string())],
                    },
                ),
                JsAst::If {
                    cond: Box::new(rt_call("isLeft", vec![JsAst::Ident(out.to_string())])),
                    then_branch: Box::new(JsAst::Block(vec![JsAst::Return(Box::new(rt_call(
                        "leftValue",
                        vec![JsAst::Ident(out.to_string())],
                    )))])),
                    else_branch: Some(Box::new(JsAst::If {
                        cond: Box::new(rt_call("isRight", vec![JsAst::Ident(out.to_string())])),
                        then_branch: Box::new(JsAst::Block(vec![assign(
                            state,
                            rt_call("rightValue", vec![JsAst::Ident(out.to_string())]),
                        )])),
                        else_branch: Some(Box::new(JsAst::If {
                            cond: Box::new(rt_call("isPair", vec![JsAst::Ident(out.to_string())])),
                            then_branch: Box::new(JsAst::Block(vec![JsAst::Return(Box::new(
                                rt_call("fst", vec![JsAst::Ident(out.to_string())]),
                            ))])),
                            else_branch: None,
                        })),
                    })),
                },
            ])),
        },
        JsAst::Return(Box::new(JsAst::Null)),
    ]);

    Ok(block)
}

/// `Iter { fuel, predicate, step, initial }` — bounded iteration.
pub fn lower_iter(
    fuel: &Term,
    predicate: &Term,
    step: &Term,
    initial: &Term,
) -> Result<JsAst, CompileError> {
    let param = input_ident();
    Ok(arrow_fn(lower_iter_applied(
        fuel, predicate, step, initial, &param,
    )?))
}

fn lower_iter_applied(
    fuel: &Term,
    predicate: &Term,
    step: &Term,
    initial: &Term,
    input: &JsAst,
) -> Result<JsAst, CompileError> {
    let fuel_val = super::lower_applied(fuel, input)?;
    let pred_fn = super::lower(predicate)?;
    let step_fn = super::lower(step)?;
    let init_val = super::lower_applied(initial, input)?;

    let state = "__state";
    let cnt = "__cnt";

    let block = JsAst::Block(vec![
        let_decl(state, init_val),
        let_decl(cnt, JsAst::Num(Number::from(0))),
        JsAst::While {
            cond: Box::new(JsAst::BinOp {
                op: JsBinOp::Lt,
                left: Box::new(JsAst::Ident(cnt.to_string())),
                right: Box::new(fuel_val),
            }),
            body: Box::new(JsAst::Block(vec![
                JsAst::If {
                    cond: Box::new(JsAst::UnaryOp {
                        op: JsUnaryOp::Not,
                        expr: Box::new(JsAst::Call {
                            callee: Box::new(pred_fn),
                            args: vec![JsAst::Ident(state.to_string())],
                        }),
                    }),
                    then_branch: Box::new(JsAst::Block(vec![JsAst::Return(Box::new(
                        JsAst::Ident(state.to_string()),
                    ))])),
                    else_branch: None,
                },
                assign(
                    state,
                    JsAst::Call {
                        callee: Box::new(step_fn),
                        args: vec![JsAst::Ident(state.to_string())],
                    },
                ),
                assign(
                    cnt,
                    JsAst::BinOp {
                        op: JsBinOp::Add,
                        left: Box::new(JsAst::Ident(cnt.to_string())),
                        right: Box::new(JsAst::Num(Number::from(1))),
                    },
                ),
            ])),
        },
        JsAst::Return(Box::new(JsAst::Ident(state.to_string()))),
    ]);

    Ok(block)
}

/// `App { callee, args }` — higher-order arrow application.
pub fn lower_app(callee: &Term, args: &[Term]) -> Result<JsAst, CompileError> {
    let param = input_ident();
    Ok(arrow_fn(lower_app_applied(callee, args, &param)?))
}

fn lower_app_applied(callee: &Term, args: &[Term], input: &JsAst) -> Result<JsAst, CompileError> {
    let callee_expr = super::lower_applied(callee, input)?;
    let arg_exprs: Vec<JsAst> = args
        .iter()
        .map(|a| super::lower_applied(a, input))
        .collect::<Result<_, _>>()?;
    Ok(JsAst::Call {
        callee: Box::new(callee_expr),
        args: arg_exprs,
    })
}
