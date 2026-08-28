use crate::core::*;

pub fn var(name: impl Into<Name>) -> Term {
    Term::Var(Var::Named(name.into()))
}

pub fn bvar(idx: usize) -> Term {
    Term::Var(Var::DeBruijn(idx))
}

pub fn universe(level: Level) -> Term {
    Term::Universe(level)
}

pub fn u0() -> Term {
    Term::Universe(Level::zero())
}

pub fn tvar(name: impl Into<Name>) -> Term {
    Term::TVar(name.into())
}

pub fn tref(name: impl Into<Name>) -> Term {
    Term::TRef(name.into())
}

pub fn alias(name: impl Into<Name>, value: Term, body: Term) -> Term {
    Term::Alias {
        name: name.into(),
        value: Box::new(value),
        body: Box::new(body),
    }
}

pub fn type_lam(name: impl Into<Name>, param_kind: Kind, body: Term) -> Term {
    Term::TypeLam {
        name: name.into(),
        param_kind,
        body: Box::new(body),
    }
}

pub fn type_app(fun: Term, arg: Term) -> Term {
    Term::TypeApp {
        fun: Box::new(fun),
        arg: Box::new(arg),
    }
}

pub fn gen_lam(type_params: impl IntoIterator<Item = (impl Into<Name>, Kind)>, body: Term) -> Term {
    Term::GenLam {
        type_params: type_params
            .into_iter()
            .map(|(n, k)| (n.into(), k))
            .collect(),
        body: Box::new(body),
    }
}

pub fn gen_app(fun: Term, type_args: impl IntoIterator<Item = Term>) -> Term {
    Term::GenApp {
        fun: Box::new(fun),
        type_args: type_args.into_iter().collect(),
    }
}

pub fn kind_type() -> Kind {
    Kind::Type
}

pub fn kfun(dom: impl Into<Box<Kind>>, cod: impl Into<Box<Kind>>) -> Kind {
    Kind::fun(dom, cod)
}

pub fn pi(name: impl Into<Name>, param: Term, body: Term) -> Term {
    Term::Pi {
        name: name.into(),
        param: Box::new(param),
        body: Box::new(body),
    }
}

pub fn lam(name: impl Into<Name>, body: Term) -> Term {
    Term::Lambda {
        name: name.into(),
        body: Box::new(body),
    }
}

pub fn app(fun: Term, arg: Term) -> Term {
    Term::App {
        fun: Box::new(fun),
        arg: Box::new(arg),
    }
}

pub fn sigma(name: impl Into<Name>, fst: Term, snd: Term) -> Term {
    Term::Sigma {
        name: name.into(),
        fst: Box::new(fst),
        snd: Box::new(snd),
    }
}

pub fn pair(fst: Term, snd: Term) -> Term {
    Term::Pair {
        fst: Box::new(fst),
        snd: Box::new(snd),
    }
}

pub fn proj1(e: Term) -> Term {
    Term::Proj1(Box::new(e))
}

pub fn proj2(e: Term) -> Term {
    Term::Proj2(Box::new(e))
}

pub fn r#let(name: impl Into<Name>, value: Term, body: Term) -> Term {
    Term::Let {
        name: name.into(),
        value: Box::new(value),
        body: Box::new(body),
    }
}

pub fn meta(id: u32) -> Term {
    Term::Meta(MetaVar(id))
}

pub fn hole(id: u32) -> Term {
    Term::Hole(HoleId(id))
}

pub fn lit_bool(b: bool) -> Term {
    Term::Literal(Literal::Bool(b))
}

pub fn lit_int(n: i64) -> Term {
    Term::Literal(Literal::Int(n))
}

pub fn lit_string(s: impl Into<String>) -> Term {
    Term::Literal(Literal::String(s.into()))
}

pub fn named_ty(name: impl Into<String>) -> Value {
    Value::Neutral(Neutral::Var(Var::Named(Name::new(name))))
}

pub fn forall(name: impl Into<Name>, param_kind: Kind, body: Value) -> Value {
    Value::Forall {
        param_name: name.into(),
        param_kind,
        body: Box::new(body),
    }
}

pub fn val_pi(
    name: impl Into<Name>,
    param: Value,
    env: Env,
    closure_name: Name,
    body: Term,
) -> Value {
    Value::Pi {
        param_name: name.into(),
        param: Box::new(param),
        closure: Closure {
            env,
            name: closure_name,
            body,
        },
    }
}

pub fn collection(name: impl Into<Name>) -> Term {
    Term::Collection(name.into())
}

pub fn collection_value(name: impl Into<Name>) -> Value {
    Value::Collection(name.into())
}

pub fn membership(value: Term, collection: Term) -> Term {
    Term::Membership {
        value: Box::new(value),
        collection: Box::new(collection),
    }
}

pub fn is(value: Term, collection: Term) -> Term {
    Term::Is {
        value: Box::new(value),
        collection: Box::new(collection),
    }
}

pub fn eq(lhs: Term, rhs: Term) -> Term {
    Term::Eq {
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

pub fn refl() -> Term {
    Term::Refl
}

pub fn typed_let(name: impl Into<Name>, value: Term, ty: Term, body: Term) -> Term {
    Term::TypedLet {
        name: name.into(),
        value: Box::new(value),
        ty: Box::new(ty),
        body: Box::new(body),
    }
}

pub fn empty() -> Term {
    Term::Empty
}

pub fn membership_value(value: Value, collection: Value) -> Value {
    Value::Membership {
        value: Box::new(value),
        collection: Box::new(collection),
    }
}

pub fn eq_value(lhs: Value, rhs: Value) -> Value {
    Value::Eq {
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

pub fn refl_value() -> Value {
    Value::Refl
}

pub fn empty_value() -> Value {
    Value::Empty
}
