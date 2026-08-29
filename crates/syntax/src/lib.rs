//! # handl-syntax
//!
//! Canonical syntax and document vocabulary configuration for the handl language.
//!
//! This crate provides `const` configuration structs for the canonical dialect.
//! Custom formats can override these values directly instead of implementing a
//! trait.

/// Interpolation and path syntax configuration for handl expressions.
///
/// The interpolation markers are string tokens (may be multi-character).
/// The path separator, call-open, and call-close are single-character tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxConfig {
    /// The interpolation prefix — e.g. `"${"` for the canonical dialect.
    pub interpolation_prefix: &'static str,

    /// The interpolation suffix — e.g. `"}"` for the canonical dialect.
    pub interpolation_suffix: &'static str,

    /// The path separator — e.g. `'.'` for the canonical dialect.
    pub path_separator: char,

    /// The call-open character — e.g. `'('` for the canonical dialect.
    pub call_open: char,

    /// The call-close character — e.g. `')'` for the canonical dialect.
    pub call_close: char,
}

/// Canonical syntax tokens for the handl language.
pub const CANONICAL_SYNTAX: SyntaxConfig = SyntaxConfig {
    interpolation_prefix: "${",
    interpolation_suffix: "}",
    path_separator: '.',
    call_open: '(',
    call_close: ')',
};

/// Reserved document key names used in handl documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentConfig {
    /// The top-level key for field declarations.
    pub fields_key: &'static str,

    /// The field-level key for computed field expressions.
    pub expr_key: &'static str,

    /// The field-level key for parameterized computed field argument type.
    pub arg_key: &'static str,

    /// The field-level key for merge field flag.
    pub merge_key: &'static str,

    /// The field-level key for scoped inheritance flag.
    pub inheritable_key: &'static str,

    /// The field-level key for immutability flag.
    pub mutable_key: &'static str,

    /// The function-level key for parameters.
    pub params_key: &'static str,

    /// The function-level key for return schema.
    pub return_key: &'static str,

    /// The function-level key for user-defined function body.
    pub body_key: &'static str,
}

/// Canonical reserved document key names for the handl language.
pub const CANONICAL_DOCUMENT: DocumentConfig = DocumentConfig {
    fields_key: "$",
    expr_key: "expr",
    arg_key: "arg",
    merge_key: "merge",
    inheritable_key: "inheritable",
    mutable_key: "mutable",
    params_key: "params",
    return_key: "return",
    body_key: "body",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_syntax_tokens() {
        assert_eq!(CANONICAL_SYNTAX.interpolation_prefix, "${");
        assert_eq!(CANONICAL_SYNTAX.interpolation_suffix, "}");
        assert_eq!(CANONICAL_SYNTAX.path_separator, '.');
        assert_eq!(CANONICAL_SYNTAX.call_open, '(');
        assert_eq!(CANONICAL_SYNTAX.call_close, ')');
    }

    #[test]
    fn canonical_document_keys() {
        assert_eq!(CANONICAL_DOCUMENT.fields_key, "$");
        assert_eq!(CANONICAL_DOCUMENT.expr_key, "expr");
        assert_eq!(CANONICAL_DOCUMENT.arg_key, "arg");
        assert_eq!(CANONICAL_DOCUMENT.merge_key, "merge");
        assert_eq!(CANONICAL_DOCUMENT.inheritable_key, "inheritable");
        assert_eq!(CANONICAL_DOCUMENT.mutable_key, "mutable");
        assert_eq!(CANONICAL_DOCUMENT.params_key, "params");
        assert_eq!(CANONICAL_DOCUMENT.return_key, "return");
        assert_eq!(CANONICAL_DOCUMENT.body_key, "body");
    }
}
