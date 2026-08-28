//! Placeholder expression, pattern, and handler types for the VM migration.
//!
//! These are compatibility types that mirror the old `handl-ast-canonical`
//! concrete enum while the platform moves to the generic `handl-ast` trait
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
    /// A handle expression: `handle expr`.
    Handle {
        /// The handled expression.
        expr: Box<Expr<V>>,
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
        /// Parameters with optional default expressions.
        params: Vec<(Pattern, Option<Expr<V>>)>,
        /// Function body.
        body: Box<Expr<V>>,
    },
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

/// Placeholder for a handler clause.
#[derive(Debug, Clone, PartialEq)]
pub struct HandlerClause;

/// Placeholder factory for constructing `Expr` nodes from a source node.
pub trait ExprFactory<E, V> {
    /// Build an `Expr<V>` from a source expression node.
    fn build(source: E) -> Expr<V>;
}
