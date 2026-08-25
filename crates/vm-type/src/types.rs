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
