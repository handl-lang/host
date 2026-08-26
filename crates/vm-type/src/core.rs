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

/// Kinds classify type-level terms.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum Kind {
    /// The kind of all ground types.
    #[default]
    Type,
    /// The kind of type constructors: `dom -> cod`.
    Fun(Box<Kind>, Box<Kind>),
}

impl Kind {
    /// Construct a function kind from `dom` to `cod`.
    pub fn fun(dom: impl Into<Box<Kind>>, cod: impl Into<Box<Kind>>) -> Self {
        Kind::Fun(dom.into(), cod.into())
    }

    /// Return the normal form of this kind.  Kinds are already in normal
    /// form, so this currently yields a clone, but it is part of the public
    /// API for Phase-1 kinding.
    pub fn normalize(&self) -> Self {
        self.clone()
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::Type => write!(f, "Type"),
            Kind::Fun(dom, cod) => write!(f, "({} -> {})", dom.as_ref(), cod.as_ref()),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TCtx {
    entries: Vec<(Name, Value, Kind)>,
    aliases: Vec<(Name, Term, Kind)>,
}

impl TCtx {
    pub fn new() -> Self {
        TCtx {
            entries: Vec::new(),
            aliases: Vec::new(),
        }
    }

    /// Extend the context with a typing entry whose kind defaults to `Kind::Type`.
    pub fn extend(&self, name: Name, ty: Value) -> Self {
        self.extend_with_kind(name, ty, Kind::Type)
    }

    /// Extend the context with a typing entry and an explicit kind.
    pub fn extend_with_kind(&self, name: Name, ty: Value, kind: Kind) -> Self {
        let mut entries = self.entries.clone();
        entries.insert(0, (name, ty, kind));
        TCtx {
            entries,
            ..self.clone()
        }
    }

    /// Extend the context with a type alias.
    pub fn add_alias(&self, name: Name, value: Term, kind: Kind) -> Self {
        let mut aliases = self.aliases.clone();
        aliases.insert(0, (name, value, kind));
        TCtx {
            entries: self.entries.clone(),
            aliases,
        }
    }

    /// Look up the type of a bound variable by De Bruijn index.
    pub fn lookup(&self, idx: usize) -> Option<&Value> {
        self.entries.get(idx).map(|(_, ty, _)| ty)
    }

    /// Look up the kind of a bound variable by De Bruijn index.
    pub fn lookup_kind(&self, idx: usize) -> Option<&Kind> {
        self.entries.get(idx).map(|(_, _, kind)| kind)
    }

    /// Look up the type of a named variable.
    pub fn lookup_by_name(&self, name: &Name) -> Option<&Value> {
        self.entries
            .iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, ty, _)| ty)
    }

    /// Look up the kind of a named variable.
    pub fn lookup_kind_by_name(&self, name: &Name) -> Option<&Kind> {
        self.entries
            .iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, _, kind)| kind)
    }

    /// Look up a type alias by name.
    pub fn lookup_alias(&self, name: &Name) -> Option<(&Term, &Kind)> {
        self.aliases
            .iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, v, k)| (v, k))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn names(&self) -> Vec<Name> {
        self.entries.iter().map(|(n, _, _)| n.clone()).collect()
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
    /// Type-level lambda: abstraction over a type of the given kind.
    TypeLam {
        name: Name,
        param_kind: Kind,
        body: Box<Term>,
    },
    /// Type-level application.
    TypeApp {
        fun: Box<Term>,
        arg: Box<Term>,
    },
    /// Named type variable (used at the type/kinding level).
    TVar(Name),
    /// Reference to a type alias.
    TRef(Name),
    /// Type alias declaration: `alias <name> = <value> in <body>`.
    Alias {
        name: Name,
        value: Box<Term>,
        body: Box<Term>,
    },
    /// Term-level generic abstraction: `ΛA. e`.
    GenLam {
        type_params: Vec<(Name, Kind)>,
        body: Box<Term>,
    },
    /// Term-level generic application: `e[T]`.
    GenApp {
        fun: Box<Term>,
        type_args: Vec<Term>,
    },
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
    /// Type constructor application stuck on a neutral head.
    TypeApp {
        fun: Box<Neutral>,
        arg: Box<Value>,
    },
    /// Named type variable (unification variable at the kinding level).
    TVar(Name),
    App {
        fun: Box<Neutral>,
        arg: Box<Value>,
    },
    Proj1(Box<Neutral>),
    Proj2(Box<Neutral>),
    Meta(MetaVar),
    Hole(HoleId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Neutral(Neutral),
    Universe(Level),
    /// Type-level lambda value.
    TypeLam {
        param_name: Name,
        param_kind: Kind,
        closure: Closure,
    },
    /// Parametric polymorphic type: `∀A:k. T`.
    Forall {
        param_name: Name,
        param_kind: Kind,
        body: Box<Value>,
    },
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
