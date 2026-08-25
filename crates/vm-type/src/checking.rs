use crate::core::*;
use crate::error::*;
use crate::reduction::{eval, quote};
use crate::unification::{MetaContext, unify};

pub fn synth(
    ctx: &mut MetaContext,
    env: &Env,
    tctx: &TCtx,
    term: &Term,
) -> Result<Value, TypeError> {
    match term {
        Term::Universe(level) => Ok(Value::Universe(level.clone().succ())),
        Term::Var(Var::DeBruijn(i)) => tctx
            .lookup(*i)
            .cloned()
            .ok_or(TypeError::UndefinedVariable(Var::DeBruijn(*i))),
        Term::Var(Var::Named(name)) => Err(TypeError::ScopeError(name.to_string())),
        Term::Literal(lit) => Ok(Value::Neutral(Neutral::Var(Var::Named(literal_type_name(
            lit,
        ))))),
        Term::Let { name, value, body } => {
            let vty = synth(ctx, env, tctx, value)?;
            let vval = eval(value, env);
            synth(
                ctx,
                &env.extend(vval),
                &tctx.extend(name.clone(), vty),
                body,
            )
        }
        Term::Pi { name, param, body } => {
            let pty = synth(ctx, env, tctx, param)?;
            let pl = universe_level(&pty)?;
            let pval = eval(param, env);
            let arg = Value::Neutral(Neutral::Var(Var::Named(name.clone())));
            let bty = synth(
                ctx,
                &env.extend(arg),
                &tctx.extend(name.clone(), pval),
                body,
            )?;
            let bl = universe_level(&bty)?;
            Ok(Value::Universe(Level::max(pl, bl)))
        }
        Term::Sigma { name, fst, snd } => {
            let fty = synth(ctx, env, tctx, fst)?;
            let fl = universe_level(&fty)?;
            let fval = eval(fst, env);
            let arg = Value::Neutral(Neutral::Var(Var::Named(name.clone())));
            let sty = synth(ctx, &env.extend(arg), &tctx.extend(name.clone(), fval), snd)?;
            let sl = universe_level(&sty)?;
            Ok(Value::Universe(Level::max(fl, sl)))
        }
        Term::Lambda { .. } => Err(TypeError::CannotSynthesize("lambda".into())),
        Term::App { fun, arg } => {
            let fty = synth(ctx, env, tctx, fun)?;
            let fty = ctx.subst.apply_value(&fty);
            match fty {
                Value::Pi { param, closure, .. } => {
                    check(ctx, env, tctx, arg, &param)?;
                    let varg = eval(arg, env);
                    Ok(closure.apply(varg))
                }
                _ => Err(TypeError::ExpectedFunction(format!(
                    "{:?}",
                    quote(&fty, &tctx.names())
                ))),
            }
        }
        Term::Proj1(e) => {
            let ety = synth(ctx, env, tctx, e)?;
            let ety = ctx.subst.apply_value(&ety);
            let ety_str = format!("{:?}", quote(&ety, &tctx.names()));
            if let Value::Sigma { fst, .. } = ety {
                Ok(*fst)
            } else {
                Err(TypeError::ExpectedSigma(ety_str))
            }
        }
        Term::Proj2(e) => {
            let ety = synth(ctx, env, tctx, e)?;
            let ety = ctx.subst.apply_value(&ety);
            let ety_str = format!("{:?}", quote(&ety, &tctx.names()));
            if let Value::Sigma { closure, .. } = ety {
                let v1 = match eval(e, env) {
                    Value::Pair { fst, .. } => *fst,
                    Value::Neutral(n) => Value::Neutral(Neutral::Proj1(Box::new(n))),
                    _ => return Err(TypeError::ExpectedSigma(ety_str)),
                };
                Ok(closure.apply(v1))
            } else {
                Err(TypeError::ExpectedSigma(ety_str))
            }
        }
        Term::Pair { .. } => Err(TypeError::CannotSynthesize("pair".into())),
        Term::Meta(_) => Err(TypeError::CannotSynthesize("meta".into())),
        Term::Hole(_) => Err(TypeError::CannotSynthesize("hole".into())),
    }
}

fn universe_level(value: &Value) -> Result<Level, TypeError> {
    match value {
        Value::Universe(l) => Ok(l.clone()),
        other => Err(TypeError::UniverseMismatch(format!("{:?}", other))),
    }
}

pub fn check(
    ctx: &mut MetaContext,
    env: &Env,
    tctx: &TCtx,
    term: &Term,
    ty: &Value,
) -> Result<(), TypeError> {
    let ty = ctx.subst.apply_value(ty);
    match (term, ty) {
        (
            Term::Lambda { name, body },
            Value::Pi {
                ref param,
                ref closure,
                ..
            },
        ) => {
            let arg = Value::Neutral(Neutral::Var(Var::Named(name.clone())));
            let body_ty = closure.apply(arg.clone());
            check(
                ctx,
                &env.extend(arg),
                &tctx.extend(name.clone(), param.as_ref().clone()),
                body.as_ref(),
                &body_ty,
            )
        }
        (
            Term::Pair { fst, snd },
            Value::Sigma {
                fst: ref a_ty,
                ref closure,
                ..
            },
        ) => {
            check(ctx, env, tctx, fst.as_ref(), a_ty.as_ref())?;
            let v1 = eval(fst.as_ref(), env);
            let b_ty = closure.apply(v1);
            check(ctx, env, tctx, snd.as_ref(), &b_ty)
        }
        (_, ty) => {
            let inferred = synth(ctx, env, tctx, term)?;
            if convertible(ctx, &inferred, &ty)? {
                Ok(())
            } else {
                Err(TypeError::TypeMismatch {
                    expected: format!("{:?}", quote(&ty, &tctx.names())),
                    found: format!("{:?}", quote(&inferred, &tctx.names())),
                })
            }
        }
    }
}

fn convertible(ctx: &mut MetaContext, from: &Value, to: &Value) -> Result<bool, TypeError> {
    let a = ctx.subst.apply_value(from);
    let b = ctx.subst.apply_value(to);
    if let (Value::Universe(l1), Value::Universe(l2)) = (&a, &b) {
        return Ok(l1.le(l2));
    }
    match unify(ctx, &a, &b) {
        Ok(()) => Ok(true),
        Err(UnificationError::Mismatch(_, _)) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

fn literal_type_name(lit: &Literal) -> Name {
    match lit {
        Literal::Bool(_) => Name::new("Bool"),
        Literal::Int(_) => Name::new("Int"),
        Literal::String(_) => Name::new("String"),
    }
}
