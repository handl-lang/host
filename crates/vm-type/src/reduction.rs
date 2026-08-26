use crate::core::*;

pub fn eval(term: &Term, env: &Env) -> Value {
    match term {
        Term::Var(Var::DeBruijn(i)) => env
            .lookup(*i)
            .cloned()
            .unwrap_or(Value::Neutral(Neutral::Var(Var::DeBruijn(*i)))),
        Term::Var(var) => Value::Neutral(Neutral::Var(var.clone())),
        Term::Universe(level) => Value::Universe(level.clone()),
        Term::TypeLam {
            name,
            param_kind,
            body,
        } => Value::TypeLam {
            param_name: name.clone(),
            param_kind: param_kind.clone(),
            closure: Closure {
                env: env.clone(),
                name: name.clone(),
                body: *body.clone(),
            },
        },
        Term::TypeApp { fun, arg } => apply_type(eval(fun, env), eval(arg, env)),
        Term::TVar(name) => Value::Neutral(Neutral::TVar(name.clone())),
        Term::TRef(name) => Value::Neutral(Neutral::TVar(name.clone())),
        Term::Alias { body, .. } => eval(body, env),
        Term::GenLam { type_params, body } if !type_params.is_empty() => {
            let (name, kind) = type_params.first().unwrap().clone();
            let rest: Vec<_> = type_params.iter().skip(1).cloned().collect();
            let mut env = env.clone();
            if !rest.is_empty() {
                env = env.extend(eval(
                    &Term::GenLam {
                        type_params: rest,
                        body: body.clone(),
                    },
                    &env,
                ));
            } else {
                env = env.extend(eval(body, &env));
            }
            Value::Forall {
                param_name: name,
                param_kind: kind,
                body: Box::new(
                    env.lookup(0)
                        .cloned()
                        .unwrap_or(Value::Neutral(Neutral::Var(Var::Named(Name::new("_"))))),
                ),
            }
        }
        Term::GenLam { body, .. } => eval(body, env),
        Term::GenApp { fun, type_args } => {
            let mut f = eval(fun, env);
            for arg in type_args {
                f = apply_forall(f, eval(arg, env));
            }
            f
        }
        Term::Pi { name, param, body } => Value::Pi {
            param_name: name.clone(),
            param: Box::new(eval(param, env)),
            closure: Closure {
                env: env.clone(),
                name: name.clone(),
                body: *body.clone(),
            },
        },
        Term::Lambda { name, body } => Value::Lambda {
            name: name.clone(),
            closure: Closure {
                env: env.clone(),
                name: name.clone(),
                body: *body.clone(),
            },
        },
        Term::App { fun, arg } => apply(eval(fun, env), eval(arg, env)),
        Term::Sigma { name, fst, snd } => Value::Sigma {
            name: name.clone(),
            fst: Box::new(eval(fst, env)),
            closure: TypeClosure {
                env: env.clone(),
                name: name.clone(),
                body: *snd.clone(),
            },
        },
        Term::Pair { fst, snd } => Value::Pair {
            fst: Box::new(eval(fst, env)),
            snd: Box::new(eval(snd, env)),
        },
        Term::Proj1(e) => match eval(e, env) {
            Value::Pair { fst, .. } => *fst,
            Value::Neutral(n) => Value::Neutral(Neutral::Proj1(Box::new(n))),
            _ => panic!("proj1 on non-pair"),
        },
        Term::Proj2(e) => match eval(e, env) {
            Value::Pair { snd, .. } => *snd,
            Value::Neutral(n) => Value::Neutral(Neutral::Proj2(Box::new(n))),
            _ => panic!("proj2 on non-pair"),
        },
        Term::Let { value, body, .. } => eval(body, &env.extend(eval(value, env))),
        Term::Meta(m) => Value::Neutral(Neutral::Meta(*m)),
        Term::Hole(h) => Value::Neutral(Neutral::Hole(*h)),
        Term::Literal(l) => Value::Literal(l.clone()),
    }
}

pub fn apply(fun: Value, arg: Value) -> Value {
    match fun {
        Value::Lambda { closure, .. } => closure.apply(arg),
        Value::Neutral(n) => Value::Neutral(Neutral::App {
            fun: Box::new(n),
            arg: Box::new(arg),
        }),
        _ => panic!("application of non-function"),
    }
}

pub fn apply_type(fun: Value, arg: Value) -> Value {
    match fun {
        Value::TypeLam { closure, .. } => closure.apply(arg),
        Value::Neutral(n) => Value::Neutral(Neutral::TypeApp {
            fun: Box::new(n),
            arg: Box::new(arg),
        }),
        _ => panic!("type application of non-type-constructor"),
    }
}

pub fn apply_forall(value: Value, arg: Value) -> Value {
    match value {
        Value::Forall {
            param_name, body, ..
        } => instantiate_tvar_value(&body, &param_name, &arg),
        Value::Neutral(n) => Value::Neutral(Neutral::TypeApp {
            fun: Box::new(n),
            arg: Box::new(arg),
        }),
        _ => panic!("generic application of non-forall"),
    }
}

pub fn instantiate_tvar_value(value: &Value, name: &Name, replacement: &Value) -> Value {
    match value {
        Value::Neutral(n) => match n {
            Neutral::TVar(n) if n == name => replacement.clone(),
            _ => Value::Neutral(instantiate_tvar_neutral(n, name, replacement)),
        },
        Value::Universe(l) => Value::Universe(l.clone()),
        Value::TypeLam {
            param_name,
            param_kind,
            closure,
        } => {
            let names = vec![param_name.clone()];
            Value::TypeLam {
                param_name: param_name.clone(),
                param_kind: param_kind.clone(),
                closure: Closure {
                    env: instantiate_tvar_env(&closure.env, name, replacement, &names),
                    name: closure.name.clone(),
                    body: instantiate_tvar_term(&closure.body, name, replacement, &names),
                },
            }
        }
        Value::Forall {
            param_name,
            param_kind,
            body,
        } => {
            if param_name == name {
                return value.clone();
            }
            Value::Forall {
                param_name: param_name.clone(),
                param_kind: param_kind.clone(),
                body: Box::new(instantiate_tvar_value(body, name, replacement)),
            }
        }
        Value::Pi {
            param_name,
            param,
            closure,
        } => {
            let names = vec![param_name.clone()];
            Value::Pi {
                param_name: param_name.clone(),
                param: Box::new(instantiate_tvar_value(param, name, replacement)),
                closure: Closure {
                    env: instantiate_tvar_env(&closure.env, name, replacement, &names),
                    name: closure.name.clone(),
                    body: instantiate_tvar_term(&closure.body, name, replacement, &names),
                },
            }
        }
        Value::Lambda { name, closure } => {
            let names = vec![name.clone()];
            Value::Lambda {
                name: name.clone(),
                closure: Closure {
                    env: instantiate_tvar_env(&closure.env, name, replacement, &names),
                    name: closure.name.clone(),
                    body: instantiate_tvar_term(&closure.body, name, replacement, &names),
                },
            }
        }
        Value::Sigma { name, fst, closure } => {
            let names = vec![name.clone()];
            Value::Sigma {
                name: name.clone(),
                fst: Box::new(instantiate_tvar_value(fst, name, replacement)),
                closure: TypeClosure {
                    env: instantiate_tvar_env(&closure.env, name, replacement, &names),
                    name: closure.name.clone(),
                    body: instantiate_tvar_term(&closure.body, name, replacement, &names),
                },
            }
        }
        Value::Pair { fst, snd } => Value::Pair {
            fst: Box::new(instantiate_tvar_value(fst, name, replacement)),
            snd: Box::new(instantiate_tvar_value(snd, name, replacement)),
        },
        Value::Literal(l) => Value::Literal(l.clone()),
    }
}

fn instantiate_tvar_env(env: &Env, name: &Name, replacement: &Value, _names: &[Name]) -> Env {
    env.entries().iter().rev().fold(Env::new(), |e, v| {
        e.extend(instantiate_tvar_value(v, name, replacement))
    })
}

fn instantiate_tvar_neutral(neutral: &Neutral, name: &Name, replacement: &Value) -> Neutral {
    match neutral {
        Neutral::Var(_) => neutral.clone(),
        Neutral::TypeApp { fun, arg } => Neutral::TypeApp {
            fun: Box::new(instantiate_tvar_neutral(fun, name, replacement)),
            arg: Box::new(instantiate_tvar_value(arg, name, replacement)),
        },
        Neutral::TVar(_) => neutral.clone(),
        Neutral::App { fun, arg } => Neutral::App {
            fun: Box::new(instantiate_tvar_neutral(fun, name, replacement)),
            arg: Box::new(instantiate_tvar_value(arg, name, replacement)),
        },
        Neutral::Proj1(n) => {
            Neutral::Proj1(Box::new(instantiate_tvar_neutral(n, name, replacement)))
        }
        Neutral::Proj2(n) => {
            Neutral::Proj2(Box::new(instantiate_tvar_neutral(n, name, replacement)))
        }
        Neutral::Meta(_) => neutral.clone(),
        Neutral::Hole(_) => neutral.clone(),
    }
}

fn instantiate_tvar_term(term: &Term, name: &Name, replacement: &Value, names: &[Name]) -> Term {
    match term {
        Term::Var(_) => term.clone(),
        Term::Universe(_) => term.clone(),
        Term::TypeLam {
            name: n,
            param_kind,
            body,
        } => {
            let mut names2 = names.to_vec();
            names2.insert(0, n.clone());
            Term::TypeLam {
                name: n.clone(),
                param_kind: param_kind.clone(),
                body: Box::new(instantiate_tvar_term(body, name, replacement, &names2)),
            }
        }
        Term::TypeApp { fun, arg } => Term::TypeApp {
            fun: Box::new(instantiate_tvar_term(fun, name, replacement, names)),
            arg: Box::new(instantiate_tvar_term(arg, name, replacement, names)),
        },
        Term::TVar(n) if n == name => quote(replacement, names),
        Term::TVar(_) => term.clone(),
        Term::TRef(_) => term.clone(),
        Term::Alias {
            name: n,
            value,
            body,
        } => Term::Alias {
            name: n.clone(),
            value: Box::new(instantiate_tvar_term(value, name, replacement, names)),
            body: Box::new(instantiate_tvar_term(body, name, replacement, names)),
        },
        Term::GenLam { type_params, body } => Term::GenLam {
            type_params: type_params.clone(),
            body: Box::new(instantiate_tvar_term(body, name, replacement, names)),
        },
        Term::GenApp { fun, type_args } => Term::GenApp {
            fun: Box::new(instantiate_tvar_term(fun, name, replacement, names)),
            type_args: type_args
                .iter()
                .map(|a| instantiate_tvar_term(a, name, replacement, names))
                .collect(),
        },
        Term::Pi {
            name: n,
            param,
            body,
        } => {
            let mut names2 = names.to_vec();
            names2.insert(0, n.clone());
            Term::Pi {
                name: n.clone(),
                param: Box::new(instantiate_tvar_term(param, name, replacement, names)),
                body: Box::new(instantiate_tvar_term(body, name, replacement, &names2)),
            }
        }
        Term::Lambda { name: n, body } => {
            let mut names2 = names.to_vec();
            names2.insert(0, n.clone());
            Term::Lambda {
                name: n.clone(),
                body: Box::new(instantiate_tvar_term(body, name, replacement, &names2)),
            }
        }
        Term::App { fun, arg } => Term::App {
            fun: Box::new(instantiate_tvar_term(fun, name, replacement, names)),
            arg: Box::new(instantiate_tvar_term(arg, name, replacement, names)),
        },
        Term::Sigma { name: n, fst, snd } => {
            let mut names2 = names.to_vec();
            names2.insert(0, n.clone());
            Term::Sigma {
                name: n.clone(),
                fst: Box::new(instantiate_tvar_term(fst, name, replacement, names)),
                snd: Box::new(instantiate_tvar_term(snd, name, replacement, &names2)),
            }
        }
        Term::Pair { fst, snd } => Term::Pair {
            fst: Box::new(instantiate_tvar_term(fst, name, replacement, names)),
            snd: Box::new(instantiate_tvar_term(snd, name, replacement, names)),
        },
        Term::Proj1(e) => Term::Proj1(Box::new(instantiate_tvar_term(e, name, replacement, names))),
        Term::Proj2(e) => Term::Proj2(Box::new(instantiate_tvar_term(e, name, replacement, names))),
        Term::Let {
            name: n,
            value,
            body,
        } => {
            let mut names2 = names.to_vec();
            names2.insert(0, n.clone());
            Term::Let {
                name: n.clone(),
                value: Box::new(instantiate_tvar_term(value, name, replacement, names)),
                body: Box::new(instantiate_tvar_term(body, name, replacement, &names2)),
            }
        }
        Term::Meta(_) => term.clone(),
        Term::Hole(_) => term.clone(),
        Term::Literal(_) => term.clone(),
    }
}

pub fn quote(value: &Value, names: &[Name]) -> Term {
    match value {
        Value::Neutral(n) => quote_neutral(n, names),
        Value::Universe(l) => Term::Universe(l.clone()),
        Value::TypeLam {
            param_name,
            param_kind,
            closure,
        } => {
            let arg = Value::Neutral(Neutral::Var(Var::Named(param_name.clone())));
            let body_val = closure.apply(arg);
            let mut names2 = names.to_vec();
            names2.insert(0, param_name.clone());
            Term::TypeLam {
                name: param_name.clone(),
                param_kind: param_kind.clone(),
                body: Box::new(quote(&body_val, &names2)),
            }
        }
        Value::Forall {
            param_name,
            param_kind,
            body,
        } => {
            let mut names2 = names.to_vec();
            names2.insert(0, param_name.clone());
            Term::TypeLam {
                name: param_name.clone(),
                param_kind: param_kind.clone(),
                body: Box::new(quote(body, &names2)),
            }
        }
        Value::Pi {
            param_name,
            param,
            closure,
        } => {
            let arg = Value::Neutral(Neutral::Var(Var::Named(param_name.clone())));
            let body_val = closure.apply(arg);
            let mut names2 = names.to_vec();
            names2.insert(0, param_name.clone());
            Term::Pi {
                name: param_name.clone(),
                param: Box::new(quote(param, names)),
                body: Box::new(quote(&body_val, &names2)),
            }
        }
        Value::Lambda { name, closure } => {
            let arg = Value::Neutral(Neutral::Var(Var::Named(name.clone())));
            let body_val = closure.apply(arg);
            let mut names2 = names.to_vec();
            names2.insert(0, name.clone());
            Term::Lambda {
                name: name.clone(),
                body: Box::new(quote(&body_val, &names2)),
            }
        }
        Value::Sigma { name, fst, closure } => {
            let arg = Value::Neutral(Neutral::Var(Var::Named(name.clone())));
            let snd_val = closure.apply(arg);
            let mut names2 = names.to_vec();
            names2.insert(0, name.clone());
            Term::Sigma {
                name: name.clone(),
                fst: Box::new(quote(fst, names)),
                snd: Box::new(quote(&snd_val, &names2)),
            }
        }
        Value::Pair { fst, snd } => Term::Pair {
            fst: Box::new(quote(fst, names)),
            snd: Box::new(quote(snd, names)),
        },
        Value::Literal(l) => Term::Literal(l.clone()),
    }
}

fn quote_neutral(n: &Neutral, names: &[Name]) -> Term {
    match n {
        Neutral::Var(Var::Named(name)) => {
            if let Some(i) = names.iter().position(|n| n == name) {
                Term::Var(Var::DeBruijn(i))
            } else {
                Term::Var(Var::Named(name.clone()))
            }
        }
        Neutral::Var(var) => Term::Var(var.clone()),
        Neutral::TypeApp { fun, arg } => Term::TypeApp {
            fun: Box::new(quote_neutral(fun, names)),
            arg: Box::new(quote(arg, names)),
        },
        Neutral::TVar(name) => {
            if let Some(i) = names.iter().position(|n| n == name) {
                Term::Var(Var::DeBruijn(i))
            } else {
                Term::TVar(name.clone())
            }
        }
        Neutral::App { fun, arg } => Term::App {
            fun: Box::new(quote_neutral(fun, names)),
            arg: Box::new(quote(arg, names)),
        },
        Neutral::Proj1(n) => Term::Proj1(Box::new(quote_neutral(n, names))),
        Neutral::Proj2(n) => Term::Proj2(Box::new(quote_neutral(n, names))),
        Neutral::Meta(m) => Term::Meta(*m),
        Neutral::Hole(h) => Term::Hole(*h),
    }
}

pub fn normalize(term: &Term) -> Term {
    quote(&eval(term, &Env::new()), &[])
}

pub fn equal(lhs: &Term, rhs: &Term) -> bool {
    normalize(lhs) == normalize(rhs)
}

/// Directional conversion: `from` is the inferred type, `to` the expected one.
pub fn convert(from: &Term, to: &Term) -> bool {
    let a = normalize(from);
    let b = normalize(to);
    if a == b {
        return true;
    }
    match (&a, &b) {
        (Term::Universe(l1), Term::Universe(l2)) => l1.le(l2),
        _ => false,
    }
}
