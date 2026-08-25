use crate::core::*;
use crate::error::UnificationError;
use crate::reduction::{apply, quote};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MetaContext {
    pub next_id: u32,
    pub subst: Substitution,
}

impl MetaContext {
    pub fn new() -> Self {
        MetaContext {
            next_id: 0,
            subst: Substitution::new(),
        }
    }

    pub fn fresh_meta(&mut self) -> MetaVar {
        let id = self.next_id;
        self.next_id += 1;
        MetaVar(id)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Substitution {
    pub entries: Vec<(MetaVar, Value)>,
}

impl Substitution {
    pub fn new() -> Self {
        Substitution {
            entries: Vec::new(),
        }
    }

    pub fn insert(&mut self, meta: MetaVar, value: Value) {
        self.entries.push((meta, value));
    }

    pub fn get(&self, meta: &MetaVar) -> Option<&Value> {
        self.entries
            .iter()
            .rev()
            .find_map(|(m, v)| if m == meta { Some(v) } else { None })
    }

    /// Apply this substitution to a value (shallow on closure bodies).
    pub fn apply_value(&self, value: &Value) -> Value {
        match value {
            Value::Neutral(n) => self.apply_neutral(n),
            Value::Universe(l) => Value::Universe(l.clone()),
            Value::Pi {
                param_name,
                param,
                closure,
            } => Value::Pi {
                param_name: param_name.clone(),
                param: Box::new(self.apply_value(param)),
                closure: self.apply_closure(closure),
            },
            Value::Lambda { name, closure } => Value::Lambda {
                name: name.clone(),
                closure: self.apply_closure(closure),
            },
            Value::Sigma { name, fst, closure } => Value::Sigma {
                name: name.clone(),
                fst: Box::new(self.apply_value(fst)),
                closure: self.apply_type_closure(closure),
            },
            Value::Pair { fst, snd } => Value::Pair {
                fst: Box::new(self.apply_value(fst)),
                snd: Box::new(self.apply_value(snd)),
            },
            Value::Literal(l) => Value::Literal(l.clone()),
        }
    }

    fn apply_env(&self, env: &Env) -> Env {
        env.entries()
            .iter()
            .rev()
            .fold(Env::new(), |e, v| e.extend(self.apply_value(v)))
    }

    fn apply_closure(&self, closure: &Closure) -> Closure {
        Closure {
            env: self.apply_env(&closure.env),
            name: closure.name.clone(),
            body: closure.body.clone(),
        }
    }

    fn apply_type_closure(&self, closure: &TypeClosure) -> TypeClosure {
        TypeClosure {
            env: self.apply_env(&closure.env),
            name: closure.name.clone(),
            body: closure.body.clone(),
        }
    }

    /// Apply the substitution to a neutral value.
    pub fn apply_neutral(&self, neutral: &Neutral) -> Value {
        match neutral {
            Neutral::Var(_) => Value::Neutral(neutral.clone()),
            Neutral::App { fun, arg } => {
                let f = self.apply_neutral(fun);
                let a = self.apply_value(arg);
                match f {
                    Value::Neutral(nf) => Value::Neutral(Neutral::App {
                        fun: Box::new(nf),
                        arg: Box::new(a),
                    }),
                    other => apply(other, a),
                }
            }
            Neutral::Proj1(n) => match self.apply_neutral(n) {
                Value::Pair { fst, .. } => *fst,
                Value::Neutral(nn) => Value::Neutral(Neutral::Proj1(Box::new(nn))),
                other => other,
            },
            Neutral::Proj2(n) => match self.apply_neutral(n) {
                Value::Pair { fst: _, snd } => *snd,
                Value::Neutral(nn) => Value::Neutral(Neutral::Proj2(Box::new(nn))),
                other => other,
            },
            Neutral::Meta(m) => match self.get(m) {
                Some(v) => self.apply_value(v),
                None => Value::Neutral(neutral.clone()),
            },
            Neutral::Hole(_) => Value::Neutral(neutral.clone()),
        }
    }

    /// Apply the substitution to a term.  `names` lists the bound variables in scope,
    /// most recent first, so that metavariable solutions can be quoted correctly.
    pub fn apply_term(&self, term: &Term, names: &[Name]) -> Term {
        match term {
            Term::Var(_) => term.clone(),
            Term::Universe(_) => term.clone(),
            Term::Pi { name, param, body } => {
                let mut names2 = names.to_vec();
                names2.insert(0, name.clone());
                Term::Pi {
                    name: name.clone(),
                    param: Box::new(self.apply_term(param, names)),
                    body: Box::new(self.apply_term(body, &names2)),
                }
            }
            Term::Lambda { name, body } => {
                let mut names2 = names.to_vec();
                names2.insert(0, name.clone());
                Term::Lambda {
                    name: name.clone(),
                    body: Box::new(self.apply_term(body, &names2)),
                }
            }
            Term::App { fun, arg } => Term::App {
                fun: Box::new(self.apply_term(fun, names)),
                arg: Box::new(self.apply_term(arg, names)),
            },
            Term::Sigma { name, fst, snd } => {
                let mut names2 = names.to_vec();
                names2.insert(0, name.clone());
                Term::Sigma {
                    name: name.clone(),
                    fst: Box::new(self.apply_term(fst, names)),
                    snd: Box::new(self.apply_term(snd, &names2)),
                }
            }
            Term::Pair { fst, snd } => Term::Pair {
                fst: Box::new(self.apply_term(fst, names)),
                snd: Box::new(self.apply_term(snd, names)),
            },
            Term::Proj1(e) => Term::Proj1(Box::new(self.apply_term(e, names))),
            Term::Proj2(e) => Term::Proj2(Box::new(self.apply_term(e, names))),
            Term::Let { name, value, body } => {
                let mut names2 = names.to_vec();
                names2.insert(0, name.clone());
                Term::Let {
                    name: name.clone(),
                    value: Box::new(self.apply_term(value, names)),
                    body: Box::new(self.apply_term(body, &names2)),
                }
            }
            Term::Meta(m) => match self.get(m) {
                Some(v) => quote(v, names),
                None => term.clone(),
            },
            Term::Hole(_) => term.clone(),
            Term::Literal(_) => term.clone(),
        }
    }

    /// Compose `other` after `self`: the resulting substitution first applies `other`,
    /// then `self`.
    pub fn compose(&self, other: &Substitution) -> Substitution {
        let mut composed = Substitution::new();
        for (m, v) in other.entries.iter() {
            composed.entries.push((*m, self.apply_value(v)));
        }
        for (m, v) in self.entries.iter() {
            composed.entries.push((*m, v.clone()));
        }
        composed
    }
}

/// Unify two values, extending the metavariable substitution in `ctx`.
pub fn unify(ctx: &mut MetaContext, lhs: &Value, rhs: &Value) -> Result<(), UnificationError> {
    let lhs = walk(ctx, lhs);
    let rhs = walk(ctx, rhs);
    if lhs == rhs {
        return Ok(());
    }
    match (lhs, rhs) {
        (Value::Neutral(Neutral::Meta(m)), v) | (v, Value::Neutral(Neutral::Meta(m))) => {
            assign(ctx, m, &v)
        }
        (Value::Universe(l1), Value::Universe(l2)) => {
            if l1.le(&l2) && l2.le(&l1) {
                Ok(())
            } else {
                Err(UnificationError::LevelMismatch(l1, l2))
            }
        }
        (Value::Neutral(n1), Value::Neutral(n2)) => unify_neutral(ctx, &n1, &n2),
        (
            Value::Pi {
                param: p1,
                closure: c1,
                ..
            },
            Value::Pi {
                param: p2,
                closure: c2,
                ..
            },
        ) => {
            unify(ctx, &p1, &p2)?;
            let arg = fresh_var(&c1.name);
            let b1 = c1.apply(arg.clone());
            let b2 = c2.apply(arg);
            unify(ctx, &b1, &b2)
        }
        (Value::Lambda { closure: c1, .. }, Value::Lambda { closure: c2, .. }) => {
            let arg = fresh_var(&c1.name);
            let b1 = c1.apply(arg.clone());
            let b2 = c2.apply(arg);
            unify(ctx, &b1, &b2)
        }
        (
            Value::Sigma {
                fst: a1,
                closure: c1,
                ..
            },
            Value::Sigma {
                fst: a2,
                closure: c2,
                ..
            },
        ) => {
            unify(ctx, &a1, &a2)?;
            let arg = fresh_var(&c1.name);
            let b1 = c1.apply(arg.clone());
            let b2 = c2.apply(arg);
            unify(ctx, &b1, &b2)
        }
        (Value::Pair { fst: a1, snd: b1 }, Value::Pair { fst: a2, snd: b2 }) => {
            unify(ctx, &a1, &a2)?;
            unify(ctx, &b1, &b2)
        }
        (Value::Literal(l1), Value::Literal(l2)) => {
            if l1 == l2 {
                Ok(())
            } else {
                Err(UnificationError::Mismatch(
                    format!("{:?}", l1),
                    format!("{:?}", l2),
                ))
            }
        }
        (l, r) => Err(UnificationError::Mismatch(
            format!("{:?}", l),
            format!("{:?}", r),
        )),
    }
}

fn fresh_var(name: &Name) -> Value {
    Value::Neutral(Neutral::Var(Var::Named(name.clone())))
}

fn walk(ctx: &MetaContext, value: &Value) -> Value {
    match value {
        Value::Neutral(Neutral::Meta(m)) => match ctx.subst.get(m) {
            Some(v) => walk(ctx, v),
            None => value.clone(),
        },
        _ => value.clone(),
    }
}

fn assign(ctx: &mut MetaContext, m: MetaVar, value: &Value) -> Result<(), UnificationError> {
    if occurs_check(ctx, m, value) {
        return Err(UnificationError::OccursCheck(m.0));
    }
    let value = walk(ctx, value);
    if let Value::Neutral(Neutral::Meta(other)) = value
        && other == m
    {
        return Ok(());
    }
    ctx.subst.insert(m, value.clone());
    Ok(())
}

fn unify_neutral(
    ctx: &mut MetaContext,
    n1: &Neutral,
    n2: &Neutral,
) -> Result<(), UnificationError> {
    if n1 == n2 {
        return Ok(());
    }
    match (n1, n2) {
        (Neutral::Var(a), Neutral::Var(b)) => {
            if a == b {
                Ok(())
            } else {
                Err(UnificationError::Mismatch(
                    format!("{:?}", a),
                    format!("{:?}", b),
                ))
            }
        }
        (Neutral::App { fun: f1, arg: a1 }, Neutral::App { fun: f2, arg: a2 }) => {
            unify_neutral(ctx, f1, f2)?;
            let a1 = walk(ctx, a1);
            let a2 = walk(ctx, a2);
            unify(ctx, &a1, &a2)
        }
        (Neutral::Proj1(a), Neutral::Proj1(b)) => unify_neutral(ctx, a, b),
        (Neutral::Proj2(a), Neutral::Proj2(b)) => unify_neutral(ctx, a, b),
        (Neutral::Hole(a), Neutral::Hole(b)) => {
            if a == b {
                Ok(())
            } else {
                Err(UnificationError::Mismatch(
                    format!("{:?}", a),
                    format!("{:?}", b),
                ))
            }
        }
        _ => Err(UnificationError::Mismatch(
            format!("{:?}", n1),
            format!("{:?}", n2),
        )),
    }
}

fn occurs_check(ctx: &MetaContext, m: MetaVar, value: &Value) -> bool {
    match value {
        Value::Neutral(n) => occurs_check_neutral(ctx, m, n),
        Value::Universe(_) | Value::Literal(_) => false,
        Value::Pi { param, closure, .. } => {
            occurs_check(ctx, m, param)
                || occurs_check_env(ctx, m, &closure.env)
                || occurs_check_term(ctx, m, &closure.body)
        }
        Value::Lambda { closure, .. } => {
            occurs_check_env(ctx, m, &closure.env) || occurs_check_term(ctx, m, &closure.body)
        }
        Value::Sigma { fst, closure, .. } => {
            occurs_check(ctx, m, fst)
                || occurs_check_env(ctx, m, &closure.env)
                || occurs_check_term(ctx, m, &closure.body)
        }
        Value::Pair { fst, snd } => occurs_check(ctx, m, fst) || occurs_check(ctx, m, snd),
    }
}

fn occurs_check_env(ctx: &MetaContext, m: MetaVar, env: &Env) -> bool {
    env.entries().iter().any(|v| occurs_check(ctx, m, v))
}

fn occurs_check_neutral(ctx: &MetaContext, m: MetaVar, neutral: &Neutral) -> bool {
    match neutral {
        Neutral::Var(_) => false,
        Neutral::App { fun, arg } => occurs_check_neutral(ctx, m, fun) || occurs_check(ctx, m, arg),
        Neutral::Proj1(n) | Neutral::Proj2(n) => occurs_check_neutral(ctx, m, n),
        Neutral::Meta(m2) => {
            if *m2 == m {
                return true;
            }
            match ctx.subst.get(m2) {
                Some(v) => occurs_check(ctx, m, v),
                None => false,
            }
        }
        Neutral::Hole(_) => false,
    }
}

fn occurs_check_term(ctx: &MetaContext, m: MetaVar, term: &Term) -> bool {
    match term {
        Term::Var(_) | Term::Universe(_) | Term::Literal(_) | Term::Hole(_) => false,
        Term::Meta(m2) => {
            if *m2 == m {
                return true;
            }
            match ctx.subst.get(m2) {
                Some(v) => occurs_check(ctx, m, v),
                None => false,
            }
        }
        Term::Pi { param, body, .. }
        | Term::Sigma {
            fst: param,
            snd: body,
            ..
        }
        | Term::Let {
            value: param, body, ..
        } => occurs_check_term(ctx, m, param) || occurs_check_term(ctx, m, body),
        Term::Lambda { body, .. } => occurs_check_term(ctx, m, body),
        Term::App { fun, arg } => occurs_check_term(ctx, m, fun) || occurs_check_term(ctx, m, arg),
        Term::Pair { fst, snd } => occurs_check_term(ctx, m, fst) || occurs_check_term(ctx, m, snd),
        Term::Proj1(e) | Term::Proj2(e) => occurs_check_term(ctx, m, e),
    }
}

/// Convenience alias used by `checking.rs`.
pub fn apply_subst_value(subst: &Substitution, value: &Value) -> Value {
    subst.apply_value(value)
}
