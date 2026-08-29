//! Placeholder expression, pattern, and handler types for the VM migration.
//!
//! These are compatibility types that mirror the old `handl-ast-canonical`
//! concrete enum while the platform moves to the generic `handl-plugin-ast` trait
//! model. They are intentionally minimal: the core VM only needs to pass
//! references to expressions around; the backend interpreter may expand the
//! variants as it ports to the new AST.

/// A concrete, tree-shaped expression node parameterized by its runtime value.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr<V> {
    /// A literal value.
    Value(V),
    /// A variable reference.
    Var(String),
    /// A named function call.
    Call {
        /// Function name.
        name: String,
        /// Function arguments.
        args: Vec<Expr<V>>,
        /// Type arguments (for generic functions).
        type_args: Vec<String>,
    },
    /// Invocation of a first-class function value.
    Apply {
        /// The function value expression.
        target: Box<Expr<V>>,
        /// The applied arguments.
        args: Vec<Expr<V>>,
    },
    /// An array literal.
    Array(Vec<Expr<V>>),
    /// An object literal.
    Object(Vec<(String, Expr<V>)>),
    /// A postfix index expression: `target[index]`.
    Index {
        /// Indexed expression.
        target: Box<Expr<V>>,
        /// Index expression.
        index: Box<Expr<V>>,
    },
    /// A postfix field access: `target.field`.
    FieldAccess {
        /// Base expression.
        target: Box<Expr<V>>,
        /// Field name.
        field: String,
    },
    /// A pipe expression: `left |> right`.
    Pipe(Box<Expr<V>>, Box<Expr<V>>),
    /// A match expression: `match subject { ... }`.
    Match {
        /// The expression being matched.
        subject: Box<Expr<V>>,
        /// Match arms: (pattern, guard, body).
        arms: Vec<(Pattern, Option<Expr<V>>, Expr<V>)>,
    },
    /// A handle expression: `handle body { ... }`.
    Handle {
        /// The handled expression body.
        body: Box<Expr<V>>,
        /// Handler clauses.
        handlers: Vec<HandlerClause<V>>,
    },
    /// An assignment expression: `target = value`.
    Assign {
        /// The binding pattern.
        target: Pattern,
        /// The assigned value.
        value: Box<Expr<V>>,
    },
    /// A function literal: `fn<...>(...) { ... }`.
    Function {
        /// Type parameters.
        type_params: Vec<String>,
        /// Parameters with optional type annotations.
        params: Vec<(Pattern, Option<Expr<V>>)>,
        /// Optional return type annotation.
        ret: Option<Box<Expr<V>>>,
        /// Function body.
        body: Box<Expr<V>>,
    },
}

impl<V> Expr<V> {
    /// Creates a `Value` expression.
    pub fn value(v: V) -> Self {
        Expr::Value(v)
    }

    /// Creates a `Var` expression.
    pub fn var(name: impl Into<String>) -> Self {
        Expr::Var(name.into())
    }

    /// Creates a `Call` expression with no type arguments.
    pub fn call(name: impl Into<String>, args: Vec<Expr<V>>) -> Self {
        Expr::Call {
            name: name.into(),
            args,
            type_args: Vec::new(),
        }
    }

    /// Creates a `Call` expression with type arguments (for generic functions).
    pub fn call_with_type_args(
        name: impl Into<String>,
        args: Vec<Expr<V>>,
        type_args: Vec<String>,
    ) -> Self {
        Expr::Call {
            name: name.into(),
            args,
            type_args,
        }
    }

    /// Creates a `Handle` expression.
    pub fn handle(body: Expr<V>, handlers: Vec<HandlerClause<V>>) -> Self {
        Expr::Handle {
            body: Box::new(body),
            handlers,
        }
    }

    /// Creates an `Assign` expression.
    pub fn assign(target: Pattern, value: Expr<V>) -> Self {
        Expr::Assign {
            target,
            value: Box::new(value),
        }
    }

    /// Creates an `Array` expression.
    pub fn array(elements: Vec<Expr<V>>) -> Self {
        Expr::Array(elements)
    }

    /// Creates an `Object` expression.
    pub fn object(entries: Vec<(String, Expr<V>)>) -> Self {
        Expr::Object(entries)
    }
}

/// A concrete pattern used in `let`/`const` bindings, match arms, and parameters.
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// A variable or identifier binding.
    Identifier(String),
    /// An array destructuring pattern.
    Array(Vec<Pattern>),
    /// An object destructuring pattern.
    Object(Vec<(String, Pattern)>),
    /// A rest (`...name`) binding.
    Rest(String),
}

impl Pattern {
    /// Returns all variable names bound by this pattern.
    pub fn bound_names(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.collect_bound_names(&mut out);
        out
    }

    fn collect_bound_names(&self, out: &mut Vec<String>) {
        match self {
            Pattern::Identifier(name) | Pattern::Rest(name) => out.push(name.clone()),
            Pattern::Array(elements) => {
                for element in elements {
                    element.collect_bound_names(out);
                }
            }
            Pattern::Object(entries) => {
                for (_, pattern) in entries {
                    pattern.collect_bound_names(out);
                }
            }
        }
    }
}

/// A handler clause in a `handle` expression.
#[derive(Debug, Clone, PartialEq)]
pub struct HandlerClause<V> {
    /// The operation name this clause matches.
    pub op_name: String,
    /// Parameter names bound when the handler fires.
    pub params: Vec<String>,
    /// The continuation name (bound to a resume function).
    pub continuation_name: String,
    /// The handler body expression.
    pub body: Expr<V>,
}

/// Placeholder factory for constructing `Expr` nodes from a source node.
pub trait ExprFactory<E, V> {
    /// Build an `Expr<V>` from a source expression node.
    fn build(source: E) -> Expr<V>;
}
