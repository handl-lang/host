use crate::core::*;
use crate::error::UnificationError;
use crate::reduction::{apply_forall, apply_type, quote};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MetaContext {
    pub next_id: u32,
    pub subst: Substitution,
    pub alias_stack: Vec<Name>,
}

impl MetaContext {
    pub fn new() -> Self {
        MetaContext {
            next_id: 0,
            subst: Substitution::new(),
            alias_stack: Vec::new(),
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
    pub tvars: Vec<(Name, Value)>,
}

impl Substitution {
    pub fn new() -> Self {
        Substitution {
            entries: Vec::new(),
            tvars: Vec::new(),
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

    pub fn insert_tvar(&mut self, name: Name, value: Value) {
        self.tvars.push((name, value));
    }

    pub fn get_tvar(&self, name: &Name) -> Option<&Value> {
        self.tvars
            .iter()
            .rev()
            .find_map(|(n, v)| if n == name { Some(v) } else { None })
    }

    /// Apply this substitution to a value (shallow on closure bodies).
    pub fn apply_value(&self, value: &Value) -> Value {
        match value {
            Value::Neutral(n) => self.apply_neutral(n),
            Value::Universe(l) => Value::Universe(l.clone()),
            Value::TypeLam {
                param_name,
                param_kind,
                closure,
            } => Value::TypeLam {
                param_name: param_name.clone(),
                param_kind: param_kind.clone(),
                closure: self.apply_closure(closure),
            },
            Value::Forall {
                param_name,
                param_kind,
                body,
            } => {
                let mut filtered = self.clone();
                filtered.tvars.retain(|(n, _)| n != param_name);
                Value::Forall {
                    param_name: param_name.clone(),
                    param_kind: param_kind.clone(),
                    body: Box::new(filtered.apply_value(body)),
                }
            }
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
            Value::Collection(name) => Value::Collection(name.clone()),
            Value::Membership { value, collection } => Value::Membership {
                value: Box::new(self.apply_value(value)),
                collection: Box::new(self.apply_value(collection)),
            },
            Value::IsEvidence => Value::IsEvidence,
            Value::Eq { lhs, rhs } => Value::Eq {
                lhs: Box::new(self.apply_value(lhs)),
                rhs: Box::new(self.apply_value(rhs)),
            },
            Value::Refl => Value::Refl,
            Value::Empty => Value::Empty,
            Value::Null => Value::Null,
            Value::Array(xs) => Value::Array(xs.iter().map(|x| self.apply_value(x)).collect()),
            Value::Object(xs) => Value::Object(
                xs.iter()
                    .map(|(n, v)| (n.clone(), self.apply_value(v)))
                    .collect(),
            ),
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
            Neutral::TypeApp { fun, arg } => {
                let f = self.apply_neutral(fun);
                let a = self.apply_value(arg);
                apply_type(f, a)
            }
            Neutral::TVar(name) => match self.get_tvar(name) {
                Some(v) => self.apply_value(v),
                None => Value::Neutral(Neutral::TVar(name.clone())),
            },
            Neutral::App { fun, arg } => {
                let f = self.apply_neutral(fun);
                let a = self.apply_value(arg);
                match f {
                    Value::Neutral(nf) => Value::Neutral(Neutral::App {
                        fun: Box::new(nf),
                        arg: Box::new(a),
                    }),
                    other => apply_type(other, a),
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
            Neutral::Is { value, collection } => Value::Neutral(Neutral::Is {
                value: Box::new(self.apply_value(value)),
                collection: Box::new(self.apply_value(collection)),
            }),
            Neutral::Membership { value, collection } => Value::Neutral(Neutral::Membership {
                value: Box::new(self.apply_value(value)),
                collection: Box::new(self.apply_value(collection)),
            }),
            Neutral::Eq { lhs, rhs } => Value::Neutral(Neutral::Eq {
                lhs: Box::new(self.apply_value(lhs)),
                rhs: Box::new(self.apply_value(rhs)),
            }),
        }
    }

    /// Apply the substitution to a term.  `names` lists the bound variables in scope,
    /// most recent first, so that metavariable solutions can be quoted correctly.
    pub fn apply_term(&self, term: &Term, names: &[Name]) -> Term {
        match term {
            Term::Var(_) => term.clone(),
            Term::Universe(_) => term.clone(),
            Term::TypeLam {
                name,
                param_kind,
                body,
            } => {
                let mut names2 = names.to_vec();
                names2.insert(0, name.clone());
                Term::TypeLam {
                    name: name.clone(),
                    param_kind: param_kind.clone(),
                    body: Box::new(self.apply_term(body, &names2)),
                }
            }
            Term::TypeApp { fun, arg } => Term::TypeApp {
                fun: Box::new(self.apply_term(fun, names)),
                arg: Box::new(self.apply_term(arg, names)),
            },
            Term::TVar(name) => match self.get_tvar(name) {
                Some(v) => quote(v, names),
                None => term.clone(),
            },
            Term::TRef(name) => match self.get_tvar(name) {
                Some(v) => quote(v, names),
                None => term.clone(),
            },
            Term::Alias { name, value, body } => Term::Alias {
                name: name.clone(),
                value: Box::new(self.apply_term(value, names)),
                body: Box::new(self.apply_term(body, names)),
            },
            Term::GenLam { type_params, body } => {
                let mut names2 = names.to_vec();
                for (n, _) in type_params.iter().rev() {
                    names2.insert(0, n.clone());
                }
                Term::GenLam {
                    type_params: type_params.clone(),
                    body: Box::new(self.apply_term(body, &names2)),
                }
            }
            Term::GenApp { fun, type_args } => Term::GenApp {
                fun: Box::new(self.apply_term(fun, names)),
                type_args: type_args
                    .iter()
                    .map(|a| self.apply_term(a, names))
                    .collect(),
            },
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
            Term::Collection(name) => Term::Collection(name.clone()),
            Term::Membership { value, collection } => Term::Membership {
                value: Box::new(self.apply_term(value, names)),
                collection: Box::new(self.apply_term(collection, names)),
            },
            Term::Is { value, collection } => Term::Is {
                value: Box::new(self.apply_term(value, names)),
                collection: Box::new(self.apply_term(collection, names)),
            },
            Term::IsEvidence => term.clone(),
            Term::Eq { lhs, rhs } => Term::Eq {
                lhs: Box::new(self.apply_term(lhs, names)),
                rhs: Box::new(self.apply_term(rhs, names)),
            },
            Term::Refl => term.clone(),
            Term::TypedLet {
                name,
                value,
                ty,
                body,
            } => {
                let mut names2 = names.to_vec();
                names2.insert(0, name.clone());
                Term::TypedLet {
                    name: name.clone(),
                    value: Box::new(self.apply_term(value, names)),
                    ty: Box::new(self.apply_term(ty, names)),
                    body: Box::new(self.apply_term(body, &names2)),
                }
            }
            Term::Empty => term.clone(),
            Term::Null => term.clone(),
            Term::Array(xs) => Term::Array(xs.iter().map(|x| self.apply_term(x, names)).collect()),
            Term::Object(xs) => Term::Object(
                xs.iter()
                    .map(|(n, x)| (n.clone(), self.apply_term(x, names)))
                    .collect(),
            ),
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
        for (n, v) in other.tvars.iter() {
            composed.tvars.push((n.clone(), self.apply_value(v)));
        }
        for (n, v) in self.tvars.iter() {
            composed.tvars.push((n.clone(), v.clone()));
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
    match (&lhs, &rhs) {
        (Value::Neutral(Neutral::Meta(m)), v) | (v, Value::Neutral(Neutral::Meta(m))) => {
            assign(ctx, *m, v)
        }
        (Value::Neutral(Neutral::TVar(n)), v) | (v, Value::Neutral(Neutral::TVar(n))) => {
            assign_tvar(ctx, n, v)
        }
        (Value::Universe(l1), Value::Universe(l2)) => {
            if l1.le(l2) && l2.le(l1) {
                Ok(())
            } else {
                Err(UnificationError::LevelMismatch(l1.clone(), l2.clone()))
            }
        }
        (Value::Neutral(n1), Value::Neutral(n2)) => unify_neutral(ctx, n1, n2),
        (
            Value::TypeLam {
                param_name,
                param_kind,
                closure: _,
            },
            Value::TypeLam {
                param_name: _,
                param_kind: k2,
                closure: _,
            },
        ) => {
            if param_kind != k2 {
                return Err(UnificationError::KindMismatch {
                    expected: param_kind.clone(),
                    found: k2.clone(),
                });
            }
            let arg = fresh_var(param_name);
            let b1 = apply_type(lhs.clone(), arg.clone());
            let b2 = apply_type(rhs.clone(), arg);
            unify(ctx, &b1, &b2)
        }
        (
            Value::Forall {
                param_name,
                param_kind,
                body: _,
            },
            Value::Forall {
                param_name: _,
                param_kind: k2,
                body: _,
            },
        ) => {
            if param_kind != k2 {
                return Err(UnificationError::KindMismatch {
                    expected: param_kind.clone(),
                    found: k2.clone(),
                });
            }
            let arg = fresh_var(param_name);
            let b1 = apply_forall(lhs.clone(), arg.clone());
            let b2 = apply_forall(rhs.clone(), arg);
            unify(ctx, &b1, &b2)
        }
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
            unify(ctx, p1, p2)?;
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
            unify(ctx, a1, a2)?;
            let arg = fresh_var(&c1.name);
            let b1 = c1.apply(arg.clone());
            let b2 = c2.apply(arg);
            unify(ctx, &b1, &b2)
        }
        (Value::Pair { fst: a1, snd: b1 }, Value::Pair { fst: a2, snd: b2 }) => {
            unify(ctx, a1, a2)?;
            unify(ctx, b1, b2)
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
        (Value::Null, Value::Null) => Ok(()),
        (Value::Array(a1), Value::Array(a2)) => {
            if a1.len() != a2.len() {
                return Err(UnificationError::Mismatch(
                    format!("array of length {}", a1.len()),
                    format!("array of length {}", a2.len()),
                ));
            }
            for (x, y) in a1.iter().zip(a2.iter()) {
                unify(ctx, x, y)?;
            }
            Ok(())
        }
        (Value::Object(o1), Value::Object(o2)) => {
            if o1.keys().ne(o2.keys()) {
                return Err(UnificationError::Mismatch(
                    format!("object keys {:?}", o1.keys().collect::<Vec<_>>()),
                    format!("object keys {:?}", o2.keys().collect::<Vec<_>>()),
                ));
            }
            for (k, v1) in o1 {
                let v2 = o2.get(k).ok_or_else(|| {
                    UnificationError::Mismatch(
                        format!("object key {}", k),
                        format!("missing key {}", k),
                    )
                })?;
                unify(ctx, v1, v2)?;
            }
            Ok(())
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
        Value::Neutral(Neutral::TVar(n)) => match ctx.subst.get_tvar(n) {
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
    ctx.subst.insert(m, value);
    Ok(())
}

fn assign_tvar(ctx: &mut MetaContext, name: &Name, value: &Value) -> Result<(), UnificationError> {
    if occurs_check_tvar(ctx, name, value) {
        return Err(UnificationError::TVarOccursCheck(name.to_string()));
    }
    let value = walk(ctx, value);
    if let Value::Neutral(Neutral::TVar(ref other)) = value
        && other == name
    {
        return Ok(());
    }
    ctx.subst.insert_tvar(name.clone(), value);
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
        (Neutral::TypeApp { fun: f1, arg: a1 }, Neutral::TypeApp { fun: f2, arg: a2 }) => {
            unify_neutral(ctx, f1, f2)?;
            let a1 = walk(ctx, a1);
            let a2 = walk(ctx, a2);
            unify(ctx, &a1, &a2)
        }
        (Neutral::App { fun: f1, arg: a1 }, Neutral::App { fun: f2, arg: a2 }) => {
            unify_neutral(ctx, f1, f2)?;
            let a1 = walk(ctx, a1);
            let a2 = walk(ctx, a2);
            unify(ctx, &a1, &a2)
        }
        (Neutral::Proj1(a), Neutral::Proj1(b)) => unify_neutral(ctx, a, b),
        (Neutral::Proj2(a), Neutral::Proj2(b)) => unify_neutral(ctx, a, b),
        (Neutral::TVar(a), Neutral::TVar(b)) => {
            if a == b {
                Ok(())
            } else {
                Err(UnificationError::Mismatch(
                    format!("{:?}", a),
                    format!("{:?}", b),
                ))
            }
        }
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
        (
            Neutral::Is {
                value: v1,
                collection: c1,
            },
            Neutral::Is {
                value: v2,
                collection: c2,
            },
        ) => {
            unify(ctx, v1, v2)?;
            unify(ctx, c1, c2)
        }
        (
            Neutral::Membership {
                value: v1,
                collection: c1,
            },
            Neutral::Membership {
                value: v2,
                collection: c2,
            },
        ) => {
            unify(ctx, v1, v2)?;
            unify(ctx, c1, c2)
        }
        (Neutral::Eq { lhs: l1, rhs: r1 }, Neutral::Eq { lhs: l2, rhs: r2 }) => {
            unify(ctx, l1, l2)?;
            unify(ctx, r1, r2)
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
        Value::TypeLam { closure, .. } => {
            occurs_check_env(ctx, m, &closure.env) || occurs_check_term(ctx, m, &closure.body)
        }
        Value::Forall { body, .. } => occurs_check(ctx, m, body),
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
        Value::Collection(_) => false,
        Value::Membership { value, collection } => {
            occurs_check(ctx, m, value) || occurs_check(ctx, m, collection)
        }
        Value::IsEvidence => false,
        Value::Eq { lhs, rhs } => occurs_check(ctx, m, lhs) || occurs_check(ctx, m, rhs),
        Value::Refl => false,
        Value::Empty => false,
        Value::Null => false,
        Value::Array(xs) => xs.iter().any(|x| occurs_check(ctx, m, x)),
        Value::Object(xs) => xs.iter().any(|(_, v)| occurs_check(ctx, m, v)),
    }
}

fn occurs_check_tvar(ctx: &MetaContext, name: &Name, value: &Value) -> bool {
    match value {
        Value::Neutral(n) => occurs_check_neutral_tvar(ctx, name, n),
        Value::Universe(_) | Value::Literal(_) => false,
        Value::TypeLam { closure, .. } => {
            occurs_check_env_tvar(ctx, name, &closure.env)
                || occurs_check_term_tvar(ctx, name, &closure.body)
        }
        Value::Forall {
            param_name, body, ..
        } => {
            if param_name == name {
                false
            } else {
                occurs_check_tvar(ctx, name, body)
            }
        }
        Value::Pi { param, closure, .. } => {
            occurs_check_tvar(ctx, name, param)
                || occurs_check_env_tvar(ctx, name, &closure.env)
                || occurs_check_term_tvar(ctx, name, &closure.body)
        }
        Value::Lambda { closure, .. } => {
            occurs_check_env_tvar(ctx, name, &closure.env)
                || occurs_check_term_tvar(ctx, name, &closure.body)
        }
        Value::Sigma { fst, closure, .. } => {
            occurs_check_tvar(ctx, name, fst)
                || occurs_check_env_tvar(ctx, name, &closure.env)
                || occurs_check_term_tvar(ctx, name, &closure.body)
        }
        Value::Pair { fst, snd } => {
            occurs_check_tvar(ctx, name, fst) || occurs_check_tvar(ctx, name, snd)
        }
        Value::Collection(_) => false,
        Value::Membership { value, collection } => {
            occurs_check_tvar(ctx, name, value) || occurs_check_tvar(ctx, name, collection)
        }
        Value::IsEvidence => false,
        Value::Eq { lhs, rhs } => {
            occurs_check_tvar(ctx, name, lhs) || occurs_check_tvar(ctx, name, rhs)
        }
        Value::Refl => false,
        Value::Empty => false,
        Value::Null => false,
        Value::Array(xs) => xs.iter().any(|x| occurs_check_tvar(ctx, name, x)),
        Value::Object(xs) => xs.iter().any(|(_, v)| occurs_check_tvar(ctx, name, v)),
    }
}

fn occurs_check_env(ctx: &MetaContext, m: MetaVar, env: &Env) -> bool {
    env.entries().iter().any(|v| occurs_check(ctx, m, v))
}

fn occurs_check_env_tvar(ctx: &MetaContext, name: &Name, env: &Env) -> bool {
    env.entries()
        .iter()
        .any(|v| occurs_check_tvar(ctx, name, v))
}

fn occurs_check_neutral(ctx: &MetaContext, m: MetaVar, neutral: &Neutral) -> bool {
    match neutral {
        Neutral::Var(_) => false,
        Neutral::TypeApp { fun, arg } => {
            occurs_check_neutral(ctx, m, fun) || occurs_check(ctx, m, arg)
        }
        Neutral::TVar(_) => false,
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
        Neutral::Is { value, collection } => {
            occurs_check(ctx, m, value) || occurs_check(ctx, m, collection)
        }
        Neutral::Membership { value, collection } => {
            occurs_check(ctx, m, value) || occurs_check(ctx, m, collection)
        }
        Neutral::Eq { lhs, rhs } => occurs_check(ctx, m, lhs) || occurs_check(ctx, m, rhs),
    }
}

fn occurs_check_neutral_tvar(ctx: &MetaContext, name: &Name, neutral: &Neutral) -> bool {
    match neutral {
        Neutral::Var(_) => false,
        Neutral::TypeApp { fun, arg } => {
            occurs_check_neutral_tvar(ctx, name, fun) || occurs_check_tvar(ctx, name, arg)
        }
        Neutral::TVar(n) => {
            if n == name {
                return true;
            }
            match ctx.subst.get_tvar(n) {
                Some(v) => occurs_check_tvar(ctx, name, v),
                None => false,
            }
        }
        Neutral::App { fun, arg } => {
            occurs_check_neutral_tvar(ctx, name, fun) || occurs_check_tvar(ctx, name, arg)
        }
        Neutral::Proj1(n) | Neutral::Proj2(n) => occurs_check_neutral_tvar(ctx, name, n),
        Neutral::Meta(_) => false,
        Neutral::Hole(_) => false,
        Neutral::Is { value, collection } => {
            occurs_check_tvar(ctx, name, value) || occurs_check_tvar(ctx, name, collection)
        }
        Neutral::Membership { value, collection } => {
            occurs_check_tvar(ctx, name, value) || occurs_check_tvar(ctx, name, collection)
        }
        Neutral::Eq { lhs, rhs } => {
            occurs_check_tvar(ctx, name, lhs) || occurs_check_tvar(ctx, name, rhs)
        }
    }
}

fn occurs_check_term(ctx: &MetaContext, m: MetaVar, term: &Term) -> bool {
    match term {
        Term::Var(_) | Term::Universe(_) | Term::Literal(_) | Term::Hole(_) => false,
        Term::TypeLam { body, .. }
        | Term::Lambda { body, .. }
        | Term::Proj1(body)
        | Term::Proj2(body) => occurs_check_term(ctx, m, body),
        Term::TypeApp { fun, arg } | Term::App { fun, arg } | Term::Pair { fst: fun, snd: arg } => {
            occurs_check_term(ctx, m, fun) || occurs_check_term(ctx, m, arg)
        }
        Term::TVar(_) | Term::TRef(_) => false,
        Term::Pi { param, body, .. }
        | Term::Sigma {
            fst: param,
            snd: body,
            ..
        }
        | Term::Let {
            value: param, body, ..
        } => occurs_check_term(ctx, m, param) || occurs_check_term(ctx, m, body),
        Term::Alias { value, body, .. } => {
            occurs_check_term(ctx, m, value) || occurs_check_term(ctx, m, body)
        }
        Term::GenLam { body, .. } => occurs_check_term(ctx, m, body),
        Term::GenApp { fun, type_args } => {
            occurs_check_term(ctx, m, fun) || type_args.iter().any(|a| occurs_check_term(ctx, m, a))
        }
        Term::Meta(m2) => {
            if *m2 == m {
                return true;
            }
            match ctx.subst.get(m2) {
                Some(v) => occurs_check(ctx, m, v),
                None => false,
            }
        }
        Term::Collection(_) => false,
        Term::Membership { value, collection } => {
            occurs_check_term(ctx, m, value) || occurs_check_term(ctx, m, collection)
        }
        Term::Is { value, collection } => {
            occurs_check_term(ctx, m, value) || occurs_check_term(ctx, m, collection)
        }
        Term::IsEvidence => false,
        Term::Eq { lhs, rhs } => occurs_check_term(ctx, m, lhs) || occurs_check_term(ctx, m, rhs),
        Term::Refl => false,
        Term::TypedLet {
            value, ty, body, ..
        } => {
            occurs_check_term(ctx, m, value)
                || occurs_check_term(ctx, m, ty)
                || occurs_check_term(ctx, m, body)
        }
        Term::Empty => false,
        Term::Null => false,
        Term::Array(xs) => xs.iter().any(|x| occurs_check_term(ctx, m, x)),
        Term::Object(xs) => xs.iter().any(|(_, x)| occurs_check_term(ctx, m, x)),
    }
}

fn occurs_check_term_tvar(ctx: &MetaContext, name: &Name, term: &Term) -> bool {
    match term {
        Term::Var(_) | Term::Universe(_) | Term::Literal(_) | Term::Hole(_) | Term::Meta(_) => {
            false
        }
        Term::TypeLam { body, .. } | Term::Lambda { body, .. } => {
            occurs_check_term_tvar(ctx, name, body)
        }
        Term::TypeApp { fun, arg } => {
            occurs_check_term_tvar(ctx, name, fun) || occurs_check_term_tvar(ctx, name, arg)
        }
        Term::TVar(n) | Term::TRef(n) => {
            if n == name {
                return true;
            }
            match ctx.subst.get_tvar(n) {
                Some(v) => occurs_check_tvar(ctx, name, v),
                None => false,
            }
        }
        Term::Pi { param, body, .. }
        | Term::Sigma {
            fst: param,
            snd: body,
            ..
        }
        | Term::App {
            fun: param,
            arg: body,
        }
        | Term::Pair {
            fst: param,
            snd: body,
        }
        | Term::Let {
            value: param, body, ..
        } => occurs_check_term_tvar(ctx, name, param) || occurs_check_term_tvar(ctx, name, body),
        Term::Alias { value, body, .. } => {
            occurs_check_term_tvar(ctx, name, value) || occurs_check_term_tvar(ctx, name, body)
        }
        Term::GenLam { body, .. } => occurs_check_term_tvar(ctx, name, body),
        Term::GenApp { fun, type_args } => {
            occurs_check_term_tvar(ctx, name, fun)
                || type_args
                    .iter()
                    .any(|a| occurs_check_term_tvar(ctx, name, a))
        }
        Term::Proj1(e) | Term::Proj2(e) => occurs_check_term_tvar(ctx, name, e),
        Term::Collection(_) => false,
        Term::Membership { value, collection } => {
            occurs_check_term_tvar(ctx, name, value)
                || occurs_check_term_tvar(ctx, name, collection)
        }
        Term::Is { value, collection } => {
            occurs_check_term_tvar(ctx, name, value)
                || occurs_check_term_tvar(ctx, name, collection)
        }
        Term::IsEvidence => false,
        Term::Eq { lhs, rhs } => {
            occurs_check_term_tvar(ctx, name, lhs) || occurs_check_term_tvar(ctx, name, rhs)
        }
        Term::Refl => false,
        Term::TypedLet {
            value, ty, body, ..
        } => {
            occurs_check_term_tvar(ctx, name, value)
                || occurs_check_term_tvar(ctx, name, ty)
                || occurs_check_term_tvar(ctx, name, body)
        }
        Term::Empty => false,
        Term::Null => false,
        Term::Array(xs) => xs.iter().any(|x| occurs_check_term_tvar(ctx, name, x)),
        Term::Object(xs) => xs.iter().any(|(_, x)| occurs_check_term_tvar(ctx, name, x)),
    }
}

/// Convenience alias used by `checking.rs`.
pub fn apply_subst_value(subst: &Substitution, value: &Value) -> Value {
    subst.apply_value(value)
}
