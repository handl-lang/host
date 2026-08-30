//! # handl-emitter-pipe
//!
//! Emitter for the **pipe operator** construct (ADR-0044 Phase 4).
//!
//! Migrates `plugin-grammar-pipe` (grammar) into a data-driven [`Language`]
//! fragment. The existing Rust rule desugars `left |> right` into a `call`
//! node (`{callee: right, args: [left]}`) at parse time at binding power
//! `(30, 31)` — left-associative. In the data-driven path we use a dedicated
//! `pipe` node kind with an `App` eval scheme, rather than desugaring to
//! `call` at parse time. This is cleaner for the data-driven model: the
//! surface construct is preserved in the AST and the eval scheme expresses
//! the application directly.
//!
//! ## Grammar
//!
//! Infix rule for `|>` at binding power `(30, 31)` — left-associative —
//! matching the existing `PipeRule` precedence (looser than comparison at 50,
//! tighter than arrow composition `>>>` at 20). Produces a `pipe` node
//! (arity 2): child 0 is the left (input), child 1 is the right (callee).
//!
//! ## Eval
//!
//! - `pipe` → `App{callee: Child(1), args: [Child(0)]}`
//!
//! Desugars `left |> right` into `call(right, [left])` — i.e. the right-hand
//! side applied to the left-hand side as its sole argument.
//!
//! ## Parity with the existing Rust rule
//!
//! The existing `PipeRule` desugars `a |> f` to `call(f, [a])` at parse time.
//! The data-driven path produces a `pipe` node with an `App` scheme that
//! evaluates to the same application. Parity is on the **evaluated value**
//! (e.g. `a |> f` evaluates to `f(a)`), not the AST node kind — the data path
//! uses a dedicated `pipe` node kind, which is expected and cleaner for the
//! data-driven model.

#![warn(missing_docs)]

use handl_emitters::{declare_token, eval_scheme, infix_binary, Emitter};
use handl_language::{LangError, Language, Scheme};

/// Emitter for the pipe operator construct.
#[derive(Debug, Default, Clone, Copy)]
pub struct PipeEmitter;

impl PipeEmitter {
    /// Create a new pipe emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for PipeEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the pipe operator's [`Language`] fragments.
///
/// # Errors
///
/// Returns [`LangError`] if fragment merge fails (collision).
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Token kind for the pipe operator.
    declare_token(&mut lang, "token.pipe")?;

    // Grammar: infix rule, left-associative at BP (30, 31).
    infix_binary(&mut lang, "grammar.pipe.infix", "token.pipe", "pipe", 30, 31)?;

    // Eval: pipe node desugars to App{callee: Child(1), args: [Child(0)]}.
    // `left |> right` evaluates to `right(left)`.
    eval_scheme(
        &mut lang,
        "eval.pipe",
        "pipe",
        Scheme::App {
            callee: Box::new(Scheme::child(1)),
            args: vec![Scheme::child(0)],
        },
    )?;

    Ok(lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_produces_expected_fragments() {
        let lang = emit().unwrap();
        assert_eq!(lang.tokens.len(), 1);
        assert_eq!(lang.grammar.len(), 1);
        assert_eq!(lang.eval.len(), 1);
    }

    #[test]
    fn emitter_trait_emit_matches_free_function() {
        let e = PipeEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }
}
