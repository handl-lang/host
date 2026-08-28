//! Compatibility `Stmt`/`StmtKind` definitions for the migration.

use handl_kit_canonical::CanonicalKind;
use handl_platform::PlatformError;
use handl_vm::{Expr, Pattern};

/// The payload of a top-level statement in a handl module.
#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind<V> {
    /// `let <pattern> = <expr>`
    Let { pattern: Pattern, value: Expr<V> },
    /// `const <pattern> = <expr>`
    Const { pattern: Pattern, value: Expr<V> },
    /// `import <source>`
    Import { source: Expr<V> },
    /// `export <value>`
    Export { value: Expr<V> },
    /// `module <name>` or `module <name> = <expr>`
    Module { name: String, body: Option<Expr<V>> },
    /// A bare expression used as a statement.
    Expr(Expr<V>),
}

/// A top-level statement in a handl module.
#[derive(Debug, Clone, PartialEq)]
pub struct Stmt<V> {
    /// The statement payload.
    pub kind: StmtKind<V>,
    /// Comment tokens appearing before the statement.
    pub leading: Vec<crate::Token<CanonicalKind>>,
    /// Comment tokens appearing immediately after the statement.
    pub trailing: Vec<crate::Token<CanonicalKind>>,
}

impl<V> Stmt<V> {
    /// Creates a `Stmt` with empty trivia.
    pub fn new(kind: StmtKind<V>) -> Self {
        Stmt {
            kind,
            leading: Vec::new(),
            trailing: Vec::new(),
        }
    }
}

/// The result of parsing a module with error recovery.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleOutput<V> {
    /// The statements that were successfully parsed.
    pub stmts: Vec<Stmt<V>>,
    /// Parse errors recovered from during parsing.
    pub diagnostics: Vec<crate::Diagnostic>,
}

/// Converts a parsed primary expression into a binding pattern.
pub fn expr_to_pattern<V>(expr: &Expr<V>) -> Result<Pattern, PlatformError> {
    match expr {
        Expr::Var(name) => Ok(Pattern::Identifier(name.clone())),
        Expr::Array(elements) => Ok(Pattern::Array(
            elements
                .iter()
                .map(expr_to_pattern)
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Expr::Object(entries) => Ok(Pattern::Object(
            entries
                .iter()
                .map(|(k, v)| expr_to_pattern(v).map(|p| (k.clone(), p)))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        _ => Err(PlatformError::Plugin("invalid pattern".to_string())),
    }
}
