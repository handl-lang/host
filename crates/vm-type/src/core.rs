use std::fmt;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum Level {
    #[default]
    Zero,
    Succ(Box<Level>),
    Max(Box<Level>, Box<Level>),
}

impl Level {
    pub fn zero() -> Self {
        Level::Zero
    }

    pub fn succ(self) -> Self {
        Level::Succ(Box::new(self))
    }

    pub fn max(a: Self, b: Self) -> Self {
        if a.le(&b) {
            b
        } else if b.le(&a) {
            a
        } else {
            Level::Max(Box::new(a), Box::new(b))
        }
    }

    /// Cumulative ordering on universe levels.
    pub fn le(&self, other: &Self) -> bool {
        match (self, other) {
            (Level::Zero, _) => true,
            (_, Level::Zero) => false,
            (Level::Succ(a), Level::Succ(b)) => a.le(b),
            (Level::Succ(_), Level::Max(r, s)) => self.le(r) || self.le(s),
            (Level::Max(a, b), r) => a.le(r) && b.le(r),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name(pub String);

impl Name {
    pub fn new(name: impl Into<String>) -> Self {
        Name(name.into())
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for Name {
    fn from(s: &str) -> Self {
        Name::new(s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NameId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MetaVar(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HoleId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Var {
    DeBruijn(usize),
    Named(Name),
}

impl fmt::Display for Var {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Var::DeBruijn(i) => write!(f, "#{}", i),
            Var::Named(name) => write!(f, "{}", name),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Literal {
    Bool(bool),
    Int(i64),
    String(String),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Env {
    entries: Vec<Value>,
}

impl Env {
    pub fn new() -> Self {
        Env {
            entries: Vec::new(),
        }
    }

    pub fn extend(&self, value: Value) -> Self {
        let mut entries = self.entries.clone();
        entries.insert(0, value);
        Env { entries }
    }

    pub fn lookup(&self, idx: usize) -> Option<&Value> {
        self.entries.get(idx)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[Value] {
        &self.entries
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TCtx {
    entries: Vec<(Name, Value)>,
}

impl TCtx {
    pub fn new() -> Self {
        TCtx {
            entries: Vec::new(),
        }
    }

    pub fn extend(&self, name: Name, ty: Value) -> Self {
        let mut entries = self.entries.clone();
        entries.insert(0, (name, ty));
        TCtx { entries }
    }

    pub fn lookup(&self, idx: usize) -> Option<&Value> {
        self.entries.get(idx).map(|(_, ty)| ty)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn names(&self) -> Vec<Name> {
        self.entries.iter().map(|(n, _)| n.clone()).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Closure {
    pub env: Env,
    pub name: Name,
    pub body: Term,
}

impl Closure {
    pub fn apply(&self, arg: Value) -> Value {
        crate::reduction::eval(&self.body, &self.env.extend(arg))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeClosure {
    pub env: Env,
    pub name: Name,
    pub body: Term,
}

impl TypeClosure {
    pub fn apply(&self, arg: Value) -> Value {
        crate::reduction::eval(&self.body, &self.env.extend(arg))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Term {
    Var(Var),
    Universe(Level),
    Pi {
        name: Name,
        param: Box<Term>,
        body: Box<Term>,
    },
    Lambda {
        name: Name,
        body: Box<Term>,
    },
    App {
        fun: Box<Term>,
        arg: Box<Term>,
    },
    Sigma {
        name: Name,
        fst: Box<Term>,
        snd: Box<Term>,
    },
    Pair {
        fst: Box<Term>,
        snd: Box<Term>,
    },
    Proj1(Box<Term>),
    Proj2(Box<Term>),
    Let {
        name: Name,
        value: Box<Term>,
        body: Box<Term>,
    },
    Meta(MetaVar),
    Hole(HoleId),
    Literal(Literal),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Neutral {
    Var(Var),
    App { fun: Box<Neutral>, arg: Box<Value> },
    Proj1(Box<Neutral>),
    Proj2(Box<Neutral>),
    Meta(MetaVar),
    Hole(HoleId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Neutral(Neutral),
    Universe(Level),
    Pi {
        param_name: Name,
        param: Box<Value>,
        closure: Closure,
    },
    Lambda {
        name: Name,
        closure: Closure,
    },
    Sigma {
        name: Name,
        fst: Box<Value>,
        closure: TypeClosure,
    },
    Pair {
        fst: Box<Value>,
        snd: Box<Value>,
    },
    Literal(Literal),
}
