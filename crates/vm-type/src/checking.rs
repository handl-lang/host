use crate::core::*;
use crate::error::*;
use crate::reduction::{apply_type, eval, instantiate_tvar_value, quote};
use crate::unification::{MetaContext, unify};

const MAX_ALIAS_DEPTH: usize = 64;

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
            let pval = synth_type(ctx, env, tctx, param)?;
            let pl = value_universe(&pval)?;
            let arg = Value::Neutral(Neutral::Var(Var::Named(name.clone())));
            let bval = synth_type(
                ctx,
                &env.extend(arg),
                &tctx.extend_with_kind(name.clone(), pval, Kind::Type),
                body,
            )?;
            let bl = value_universe(&bval)?;
            Ok(Value::Universe(Level::max(pl, bl)))
        }
        Term::Sigma { name, fst, snd } => {
            let fval = synth_type(ctx, env, tctx, fst)?;
            let fl = value_universe(&fval)?;
            let arg = Value::Neutral(Neutral::Var(Var::Named(name.clone())));
            let sval = synth_type(
                ctx,
                &env.extend(arg),
                &tctx.extend_with_kind(name.clone(), fval, Kind::Type),
                snd,
            )?;
            let sl = value_universe(&sval)?;
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
        Term::TVar(_) | Term::TRef(_) | Term::TypeApp { .. } => {
            let val = synth_type(ctx, env, tctx, term)?;
            let level = value_universe(&val)?;
            Ok(Value::Universe(level))
        }
        Term::TypeLam { .. } => Err(TypeError::CannotSynthesize("type lambda".into())),
        Term::Alias { name, value, body } => {
            let alias_kind = synth_kind(ctx, env, tctx, value)?;
            let tctx2 = tctx.add_alias(name.clone(), value.as_ref().clone(), alias_kind);
            synth(ctx, env, &tctx2, body)
        }
        Term::GenLam { type_params, body } => {
            let mut tctx2 = tctx.clone();
            for (name, kind) in type_params {
                tctx2 = tctx2.extend_with_kind(
                    name.clone(),
                    Value::Neutral(Neutral::TVar(name.clone())),
                    kind.clone(),
                );
            }
            let body_ty = synth(ctx, env, &tctx2, body)?;
            let mut result = body_ty;
            for (name, kind) in type_params.iter().rev() {
                result = Value::Forall {
                    param_name: name.clone(),
                    param_kind: kind.clone(),
                    body: Box::new(result),
                };
            }
            Ok(result)
        }
        Term::GenApp { fun, type_args } => {
            let mut fty = synth(ctx, env, tctx, fun)?;
            for (idx, arg) in type_args.iter().enumerate() {
                let current = ctx.subst.apply_value(&fty);
                match current {
                    Value::Forall {
                        param_name,
                        param_kind,
                        body,
                    } => {
                        let arg_kind = synth_kind(ctx, env, tctx, arg)?;
                        if arg_kind.normalize() != param_kind.normalize() {
                            return Err(TypeError::KindMismatch {
                                expected: format!("{:?}", param_kind),
                                found: format!("{:?}", arg_kind),
                            });
                        }
                        let arg_value = synth_type(ctx, env, tctx, arg)?;
                        fty = instantiate_tvar_value(&body, &param_name, &arg_value);
                    }
                    _ => {
                        return Err(TypeError::ExpectedFunction(format!(
                            "generic argument {} on {:?}",
                            idx,
                            quote(&fty, &tctx.names())
                        )));
                    }
                }
            }
            Ok(fty)
        }
    }
}

/// Synthesize the value denoted by a type-level term.
pub fn synth_type(
    ctx: &mut MetaContext,
    env: &Env,
    tctx: &TCtx,
    term: &Term,
) -> Result<Value, TypeError> {
    match term {
        Term::Universe(level) => Ok(Value::Universe(level.clone())),
        Term::Var(Var::DeBruijn(i)) => env
            .lookup(*i)
            .cloned()
            .ok_or(TypeError::UndefinedVariable(Var::DeBruijn(*i))),
        Term::Var(Var::Named(name)) => Err(TypeError::ScopeError(name.to_string())),
        Term::TVar(name) => tctx
            .lookup_by_name(name)
            .cloned()
            .ok_or_else(|| TypeError::UndefinedTypeVariable(name.clone())),
        Term::TRef(name) => expand_alias(ctx, env, tctx, name),
        Term::TypeLam { .. } => Ok(eval(term, env)),
        Term::TypeApp { fun, arg } => {
            let fval = synth_type(ctx, env, tctx, fun)?;
            let aval = synth_type(ctx, env, tctx, arg)?;
            let fkind = synth_kind(ctx, env, tctx, fun)?;
            let akind = synth_kind(ctx, env, tctx, arg)?;
            match fkind.normalize() {
                Kind::Fun(dom, _cod) => {
                    if dom.normalize() == akind.normalize() {
                        Ok(apply_type(fval, aval))
                    } else {
                        Err(TypeError::KindMismatch {
                            expected: format!("{:?}", dom),
                            found: format!("{:?}", akind),
                        })
                    }
                }
                other => Err(TypeError::ExpectedFunctionKind(format!("{:?}", other))),
            }
        }
        Term::Pi { name, param, body } => {
            let pval = synth_type(ctx, env, tctx, param)?;
            let arg = Value::Neutral(Neutral::Var(Var::Named(name.clone())));
            let bval = synth_type(
                ctx,
                &env.extend(arg),
                &tctx.extend_with_kind(name.clone(), pval.clone(), Kind::Type),
                body,
            )?;
            Ok(Value::Pi {
                param_name: name.clone(),
                param: Box::new(pval),
                closure: Closure {
                    env: env.clone(),
                    name: name.clone(),
                    body: quote(&bval, std::slice::from_ref(name)),
                },
            })
        }
        Term::Sigma { name, fst, snd } => {
            let fval = synth_type(ctx, env, tctx, fst)?;
            let arg = Value::Neutral(Neutral::Var(Var::Named(name.clone())));
            let sval = synth_type(
                ctx,
                &env.extend(arg),
                &tctx.extend_with_kind(name.clone(), fval.clone(), Kind::Type),
                snd,
            )?;
            Ok(Value::Sigma {
                name: name.clone(),
                fst: Box::new(fval),
                closure: TypeClosure {
                    env: env.clone(),
                    name: name.clone(),
                    body: quote(&sval, std::slice::from_ref(name)),
                },
            })
        }
        other => Err(TypeError::UniverseMismatch(format!(
            "expected a type, found {:?}",
            other
        ))),
    }
}

/// Check that a type-level term has the expected kind.
pub fn check_type(
    ctx: &mut MetaContext,
    env: &Env,
    tctx: &TCtx,
    term: &Term,
    kind: &Kind,
) -> Result<Value, TypeError> {
    let val = synth_type(ctx, env, tctx, term)?;
    let inferred = synth_kind(ctx, env, tctx, term)?;
    if inferred.normalize() == kind.normalize() {
        Ok(val)
    } else {
        Err(TypeError::KindMismatch {
            expected: format!("{:?}", kind),
            found: format!("{:?}", inferred),
        })
    }
}

fn expand_alias(
    ctx: &mut MetaContext,
    env: &Env,
    tctx: &TCtx,
    name: &Name,
) -> Result<Value, TypeError> {
    if ctx.alias_stack.contains(name) {
        return Err(TypeError::AliasCycle(name.clone()));
    }
    if ctx.alias_stack.len() >= MAX_ALIAS_DEPTH {
        return Err(TypeError::AliasDepthLimit(name.clone()));
    }
    let (term, _) = tctx
        .lookup_alias(name)
        .ok_or_else(|| TypeError::AliasNotFound(name.clone()))?;
    ctx.alias_stack.push(name.clone());
    let value = synth_type(ctx, env, tctx, term)?;
    ctx.alias_stack.pop();
    Ok(value)
}

fn value_universe(value: &Value) -> Result<Level, TypeError> {
    match value {
        Value::Universe(l) => Ok(l.clone().succ()),
        Value::Neutral(_) => Ok(Level::zero()),
        Value::Pi { param, closure, .. } => {
            let pl = value_universe(param)?;
            let arg = Value::Neutral(Neutral::Var(Var::Named(closure.name.clone())));
            let b = closure.apply(arg);
            let bl = value_universe(&b)?;
            Ok(Level::max(pl, bl))
        }
        Value::Sigma { fst, closure, .. } => {
            let fl = value_universe(fst)?;
            let arg = Value::Neutral(Neutral::Var(Var::Named(closure.name.clone())));
            let s = closure.apply(arg);
            let sl = value_universe(&s)?;
            Ok(Level::max(fl, sl))
        }
        Value::Forall {
            param_kind, body, ..
        } => {
            if *param_kind != Kind::Type {
                return Err(TypeError::UniverseMismatch(format!(
                    "forall over non-type kind {:?}",
                    param_kind
                )));
            }
            let bl = value_universe(body)?;
            Ok(Level::max(Level::zero(), bl))
        }
        other => Err(TypeError::UniverseMismatch(format!(
            "expected a universe, found {:?}",
            other
        ))),
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
        (Term::GenLam { type_params, body }, ty) => {
            let mut current = ty.clone();
            let mut tctx2 = tctx.clone();
            for (name, kind) in type_params {
                match current {
                    Value::Forall {
                        param_name,
                        param_kind,
                        body: forall_body,
                    } => {
                        if *kind != param_kind {
                            return Err(TypeError::KindMismatch {
                                expected: format!("{:?}", param_kind),
                                found: format!("{:?}", kind),
                            });
                        }
                        let placeholder = Value::Neutral(Neutral::TVar(name.clone()));
                        let forall_body = if *name != param_name {
                            instantiate_tvar_value(&forall_body, &param_name, &placeholder)
                        } else {
                            *forall_body
                        };
                        tctx2 = tctx2.extend_with_kind(name.clone(), placeholder, kind.clone());
                        current = forall_body;
                    }
                    _ => {
                        return Err(TypeError::TypeMismatch {
                            expected: format!("{:?}", quote(&current, &tctx2.names())),
                            found: format!("generic with {} parameters", type_params.len()),
                        });
                    }
                }
            }
            check(ctx, env, &tctx2, body, &current)
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

/// Synthesize the kind of a type-level term.
pub fn synth_kind(
    _ctx: &mut MetaContext,
    _env: &Env,
    tctx: &TCtx,
    term: &Term,
) -> Result<Kind, TypeError> {
    match term {
        Term::Universe(_) => Ok(Kind::Type),
        Term::TVar(name) => tctx
            .lookup_kind_by_name(name)
            .cloned()
            .ok_or_else(|| TypeError::UndefinedTypeVariable(name.clone())),
        Term::TRef(name) => tctx
            .lookup_alias(name)
            .map(|(_, k)| k.clone())
            .ok_or_else(|| TypeError::AliasNotFound(name.clone())),
        Term::Var(Var::DeBruijn(i)) => tctx
            .lookup_kind(*i)
            .cloned()
            .ok_or(TypeError::UndefinedTypeVariable(Name::new("#?"))),
        Term::Var(Var::Named(name)) => tctx
            .lookup_kind_by_name(name)
            .cloned()
            .ok_or_else(|| TypeError::UndefinedTypeVariable(name.clone())),
        Term::TypeLam {
            name,
            param_kind,
            body,
        } => {
            let placeholder = Value::Neutral(Neutral::TVar(name.clone()));
            let tctx = tctx.extend_with_kind(name.clone(), placeholder, param_kind.clone());
            let body_kind = synth_kind(_ctx, _env, &tctx, body)?;
            Ok(Kind::fun(param_kind.clone(), body_kind))
        }
        Term::TypeApp { fun, arg } => {
            let fun_kind = synth_kind(_ctx, _env, tctx, fun)?;
            let arg_kind = synth_kind(_ctx, _env, tctx, arg)?;
            match fun_kind.normalize() {
                Kind::Fun(dom, cod) => {
                    if dom.normalize() == arg_kind.normalize() {
                        Ok(*cod)
                    } else {
                        Err(TypeError::KindMismatch {
                            expected: format!("{:?}", dom),
                            found: format!("{:?}", arg_kind),
                        })
                    }
                }
                other => Err(TypeError::ExpectedFunctionKind(format!("{:?}", other))),
            }
        }
        Term::Pi { .. } | Term::Sigma { .. } => Ok(Kind::Type),
        _ => Err(TypeError::CannotSynthesize("kind".into())),
    }
}

/// Check that a type-level term has the expected kind.
pub fn check_kind(
    ctx: &mut MetaContext,
    env: &Env,
    tctx: &TCtx,
    term: &Term,
    kind: &Kind,
) -> Result<(), TypeError> {
    let inferred = synth_kind(ctx, env, tctx, term)?;
    if inferred.normalize() == kind.normalize() {
        Ok(())
    } else {
        Err(TypeError::KindMismatch {
            expected: format!("{:?}", kind),
            found: format!("{:?}", inferred),
        })
    }
}

fn literal_type_name(lit: &Literal) -> Name {
    match lit {
        Literal::Bool(_) => Name::new("Bool"),
        Literal::Int(_) => Name::new("Int"),
        Literal::String(_) => Name::new("String"),
    }
}
