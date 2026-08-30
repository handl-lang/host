//! # handl-emitters
//!
//! The emitter infrastructure framework for ADR-0044 Phase 4.
//!
//! ADR-0044 §6 specifies that each `plugin-grammar-*` / `plugin-eval-*` Rust
//! rule crate becomes a small Rust **emitter** that produces [`Language`]
//! fragments (grammar rows + eval schemes). The emitters are a temporary
//! scaffold: once all constructs emit and verify, they are consolidated into
//! authored data files and the emitters are deleted.
//!
//! This crate defines the [`Emitter`] trait — the contract every migrated rule
//! crate implements — plus helper functions for building common fragment
//! patterns (prefix atoms, infix operators, eval schemes). Individual emitters
//! are small: they call these helpers and return a [`Language`].
//!
//! ## Data-driven payload convention
//!
//! Emitters produce data for the data-driven engines (`PrattEngine`,
//! `TreeWalkEngine`). Those engines use a specific payload convention that
//! differs from the existing Rust rule crates:
//!
//! - **Atoms** (arity 0): payload is the token's interpreted value (number if
//!   the token kind id contains `"number"`, else the token text as a string).
//! - **Compound nodes** (arity > 0): payload is `{"children": [idx0, ...]}` of
//!   arena indices.
//!
//! Eval schemes use [`Scheme::Child`]`n)` to reference children and
//! `"$payload"` to read data from the current node's payload. See the
//! tree-walk engine docs for the full convention.

#![warn(missing_docs)]

use handl_language::{
    AstShapeRef, EvalSchemeDef, GrammarPosition, GrammarRow, Language, Scheme, TokenKindDef,
};

/// The contract every migrated rule crate implements.
///
/// An emitter produces a [`Language`] fragment (or set of fragments) encoding
/// the same semantics as the existing Rust `Behavior`/`ParserBehavior` impl.
/// The returned [`Language`] is merged into a complete language by a kit
/// (Phase 5) or directly in tests.
///
/// # Errors
///
/// Returns [`handl_language::LangError`] if fragment construction or merge
/// fails (e.g. a collision). Most emitters build fragments infallibly and
/// return `Ok(...)`; the `Result` is for uniformity with the merge path.
pub trait Emitter {
    /// Emit this emitter's [`Language`] fragments.
    ///
    /// # Errors
    ///
    /// Returns [`handl_language::LangError`] on fragment merge failure.
    fn emit(&self) -> Result<Language, handl_language::LangError>;
}

// ---------------------------------------------------------------------------
// Helper functions for building common fragment patterns
// ---------------------------------------------------------------------------

/// Declare a token kind in a [`Language`].
///
/// Convenience wrapper around `TokenKindDef::new` + `lang.tokens.merge`.
///
/// # Errors
///
/// Returns [`handl_language::LangError`] if the merge fails (collision).
pub fn declare_token(lang: &mut Language, id: &str) -> Result<(), handl_language::LangError> {
    lang.tokens.merge(TokenKindDef::new(id))
}

/// Add a prefix grammar row for an atom (arity 0) that produces a node of
/// the given `node_kind`.
///
/// The Pratt engine will set the atom's payload to the token's interpreted
/// value (number or string). This matches the existing `IdentifierRule`
/// (kind `"var"`) and `ValueRule` (kind `"value"`) prefix rules.
///
/// # Errors
///
/// Returns [`handl_language::LangError`] if the merge fails.
pub fn prefix_atom(
    lang: &mut Language,
    id: &str,
    token: &str,
    node_kind: &str,
) -> Result<(), handl_language::LangError> {
    lang.grammar.merge(GrammarRow::new(
        id,
        token,
        GrammarPosition::Prefix,
        AstShapeRef::new(node_kind, 0),
    ))
}

/// Add a prefix grammar row for a compound prefix operator (arity > 0) that
/// produces a node of the given `node_kind` with `arity` children.
///
/// The Pratt engine parses `arity` operands and sets the payload to
/// `{"children": [idx0, ...]}`.
///
/// `prefix_rbp` is the right binding power used when parsing the operands
/// (stored as the `left` component of `binding_power` for prefix rows).
///
/// # Errors
///
/// Returns [`handl_language::LangError`] if the merge fails.
pub fn prefix_compound(
    lang: &mut Language,
    id: &str,
    token: &str,
    node_kind: &str,
    arity: usize,
    prefix_rbp: u8,
) -> Result<(), handl_language::LangError> {
    lang.grammar.merge(
        GrammarRow::new(
            id,
            token,
            GrammarPosition::Prefix,
            AstShapeRef::new(node_kind, arity),
        )
        .with_binding_power(prefix_rbp, 0),
    )
}

/// Add an infix grammar row for a binary operator (arity 2) that produces a
/// node of the given `node_kind`.
///
/// The Pratt engine sets the payload to `{"children": [left_idx, right_idx]}`.
///
/// # Errors
///
/// Returns [`handl_language::LangError`] if the merge fails.
pub fn infix_binary(
    lang: &mut Language,
    id: &str,
    token: &str,
    node_kind: &str,
    lbp: u8,
    rbp: u8,
) -> Result<(), handl_language::LangError> {
    lang.grammar.merge(
        GrammarRow::new(
            id,
            token,
            GrammarPosition::Infix,
            AstShapeRef::new(node_kind, 2),
        )
        .with_binding_power(lbp, rbp),
    )
}

/// Add an eval scheme mapping `node_kind` to a [`Scheme`].
///
/// # Errors
///
/// Returns [`handl_language::LangError`] if the merge fails (collision).
pub fn eval_scheme(
    lang: &mut Language,
    id: &str,
    node_kind: &str,
    scheme: Scheme,
) -> Result<(), handl_language::LangError> {
    lang.eval.merge(EvalSchemeDef::new(id, node_kind, scheme))
}

/// Build a `Prim` scheme for a binary arithmetic primitive over two children.
///
/// Convenience: `prim_binary("add")` →
/// `Prim { op: "add", args: [Child(0), Child(1)] }`.
pub fn prim_binary(op: &str) -> Scheme {
    Scheme::prim(op, vec![Scheme::child(0), Scheme::child(1)])
}

/// Build an `App` scheme applying `Child(0)` (the callee) to the remaining
/// children as arguments: `App { callee: Child(0), args: [Child(1), ...] }`.
///
/// For a call node with N children, child 0 is the callee and children 1..N
/// are the arguments.
pub fn app_call(num_args: usize) -> Scheme {
    let args: Vec<Scheme> = (1..=num_args).map(Scheme::child).collect();
    Scheme::App {
        callee: Box::new(Scheme::child(0)),
        args,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declare_token_adds_token_kind() {
        let mut lang = Language::new();
        declare_token(&mut lang, "token.number").unwrap();
        assert_eq!(lang.tokens.len(), 1);
    }

    #[test]
    fn prefix_atom_adds_grammar_row() {
        let mut lang = Language::new();
        declare_token(&mut lang, "token.identifier").unwrap();
        prefix_atom(&mut lang, "g.ident", "token.identifier", "var").unwrap();
        assert_eq!(lang.grammar.len(), 1);
    }

    #[test]
    fn prefix_compound_adds_grammar_row_with_binding_power() {
        let mut lang = Language::new();
        declare_token(&mut lang, "token.lparen").unwrap();
        prefix_compound(&mut lang, "g.paren", "token.lparen", "paren", 1, 0).unwrap();
        assert_eq!(lang.grammar.len(), 1);
    }

    #[test]
    fn infix_binary_adds_grammar_row_with_binding_power() {
        let mut lang = Language::new();
        declare_token(&mut lang, "token.plus").unwrap();
        infix_binary(&mut lang, "g.plus", "token.plus", "binary.add", 55, 56).unwrap();
        assert_eq!(lang.grammar.len(), 1);
    }

    #[test]
    fn eval_scheme_adds_eval_def() {
        let mut lang = Language::new();
        eval_scheme(
            &mut lang,
            "e.add",
            "binary.add",
            prim_binary("add"),
        )
        .unwrap();
        assert_eq!(lang.eval.len(), 1);
    }

    #[test]
    fn prim_binary_builds_correct_scheme() {
        let s = prim_binary("add");
        assert_eq!(
            s,
            Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)])
        );
    }

    #[test]
    fn app_call_builds_correct_scheme() {
        let s = app_call(2);
        assert_eq!(
            s,
            Scheme::App {
                callee: Box::new(Scheme::child(0)),
                args: vec![Scheme::child(1), Scheme::child(2)],
            }
        );
    }

    #[test]
    fn app_call_zero_args_is_just_callee() {
        let s = app_call(0);
        assert_eq!(
            s,
            Scheme::App {
                callee: Box::new(Scheme::child(0)),
                args: vec![],
            }
        );
    }

    #[test]
    fn declare_token_duplicate_is_idempotent() {
        // TokenKindDef is a union (idempotent) fragment type: re-declaring a
        // token kind that already exists is a no-op (first declaration wins),
        // not a collision. This lets many emitters re-declare shared tokens
        // like `token.lparen` without conflicting.
        let mut lang = Language::new();
        declare_token(&mut lang, "token.number").unwrap();
        declare_token(&mut lang, "token.number").unwrap();
        assert_eq!(lang.tokens.len(), 1);
    }
}
