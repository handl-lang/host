use crate::core::*;

pub fn eval(term: &Term, env: &Env) -> Value {
    match term {
        Term::Var(Var::DeBruijn(i)) => env
            .lookup(*i)
            .cloned()
            .unwrap_or(Value::Neutral(Neutral::Var(Var::DeBruijn(*i)))),
        Term::Var(var) => Value::Neutral(Neutral::Var(var.clone())),
        Term::Universe(level) => Value::Universe(level.clone()),
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

pub fn quote(value: &Value, names: &[Name]) -> Term {
    match value {
        Value::Neutral(n) => quote_neutral(n, names),
        Value::Universe(l) => Term::Universe(l.clone()),
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
