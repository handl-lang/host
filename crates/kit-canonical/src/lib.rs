//! Canonical kit backed by the data-driven runtime.
//!
//! `Kit` assembles a [`Runtime`] from the data-driven canonical kit
//! (`handl-kit-data-canonical`) and exposes staged `lex`/`parse`/`eval`
//! methods over `handl_language` types (`Tokens`, `Ast`, `Value`).
//!
//! This is the ADR-0044 migration of `kit-canonical` from the old Rust
//! plugin-host assembly (which wired together dozens of `plugin-token-*`,
//! `plugin-grammar-*`, `plugin-eval-*`, and `plugin-ast-*` crates) to the
//! data-driven path: a single [`Runtime`] built from merged data fragments.

#![warn(missing_docs)]

use handl_kit_data_canonical;
use handl_language::{Ast, LangError, Source, Span, Tokens, Value};
use handl_runtime::meta::{canonical_meta_host_functions, MetaHostFunctions};
use handl_runtime::Runtime;
use std::rc::Rc;

/// Kit manifest loading and the default canonical manifest.
pub mod kit_manifest;
pub use kit_manifest::{canonical_manifest, KitManifest};

/// A token annotated with its source span.
///
/// The data-driven lexer (`handl_language::Token`) carries only `kind` and
/// `text` — no span. This helper type augments a token with the byte offsets
/// of its match in the source, which LSP/tooling hosts need for range-based
/// features (hover, goto-definition, semantic tokens).
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    /// The token kind id (e.g. `"token.identifier"`).
    pub kind: String,
    /// The matched source text.
    pub text: String,
    /// The byte span of the match in the source.
    pub span: Span,
}

/// The canonical kit: a thin wrapper around the data-driven [`Runtime`].
///
/// [`Kit::new`] assembles the data-driven canonical language
/// (`handl_kit_data_canonical::assemble`) and binds it to the Pratt parse
/// engine and tree-walk eval engine via [`Runtime::from_config`]. It also
/// wires the meta host functions (`eval`/`self`/`compile`) so a running
/// handl program can call `eval(lang, src)`, `runtime.self()`, and
/// `compile(lang, target)` (ADR-0041 Phase 2–3).
///
/// All staged methods (`lex`, `parse`, `eval`, `run`) delegate to the
/// underlying [`Runtime`].
pub struct Kit {
    /// The runtime, borrowing the leaked meta host functions for dispatch.
    runtime: Runtime<'static>,
}

impl std::fmt::Debug for Kit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Kit")
            .field("runtime", &self.runtime)
            .finish()
    }
}

impl Kit {
    /// Build the canonical kit from the data-driven canonical language.
    ///
    /// Assembles all Phase 4 data fragments via
    /// `handl_kit_data_canonical::assemble` and binds the resulting
    /// [`Language`](handl_language::Language) to the data-driven lexer,
    /// Pratt parser, and tree-walk evaluator. Wires meta host functions
    /// (`eval`/`self`/`compile`) so a running handl program can call them
    /// (ADR-0041 Phase 2–3).
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] if fragment assembly or engine binding fails.
    pub fn new() -> Result<Self, LangError> {
        let kit = handl_kit_data_canonical::assemble()?;
        let lang = kit.lang().clone();
        let config = kit.runtime_config().clone();
        let meta = canonical_meta_host_functions(lang.clone(), config.clone());
        // Leak the MetaHostFunctions to obtain a &'static reference. It lives
        // for the process lifetime — acceptable for a Kit that is typically
        // created once at startup. This avoids self-referential borrow issues:
        // the `Runtime<'static>` borrows the meta registry, and the `Kit`
        // owns the runtime, so the meta cannot also be owned by the `Kit`
        // without a self-referential struct.
        let meta_static: &'static MetaHostFunctions = Box::leak(Box::new(
            Rc::try_unwrap(meta).unwrap_or_else(|rc| (*rc).clone()),
        ));
        // Inject `self` as a global variable bound to the canonical
        // `Language` value (ADR-0041 Phase 3: `self` is an ambient variable,
        // not a function call). A `Language` is now a first-class value
        // carrying the actual `handl_language::Language` data.
        let self_val = handl_language::language_value(&lang);
        let mut globals = std::collections::HashMap::new();
        globals.insert("self".to_string(), self_val);
        let runtime = Runtime::from_config_with_globals(
            lang,
            &config,
            Some(meta_static),
            Some(&globals),
        )?;
        Ok(Kit { runtime })
    }

    /// Build a `Kit` from a `KitManifest`.
    ///
    /// This is a compatibility wrapper: in the data-driven model the manifest
    /// is replaced by the data fragment registry, so this always assembles the
    /// full canonical language regardless of the manifest contents. The
    /// manifest parameter is retained for API compatibility with downstream
    /// callers that still pass a `KitManifest`.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] if fragment assembly or engine binding fails.
    pub fn from_manifest(_manifest: &KitManifest) -> Result<Self, LangError> {
        Self::new()
    }

    /// Lex `input` into a token stream using the data-driven lexer.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on lex failure (e.g. unrecognized character,
    /// unterminated string).
    pub fn lex(&self, input: &str) -> Result<Tokens, LangError> {
        self.runtime.lex(&Source::new(input))
    }

    /// Lex `input` into a token stream with source spans.
    ///
    /// The data-driven lexer produces tokens without spans. This method
    /// re-computes the byte span of each token by scanning the source text,
    /// skipping whitespace between tokens.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on lex failure.
    pub fn lex_with_spans(&self, input: &str) -> Result<Vec<SpannedToken>, LangError> {
        let tokens = self.lex(input)?;
        let source = input;
        let mut pos: usize = 0;
        let mut result = Vec::with_capacity(tokens.as_slice().len());
        for token in tokens.as_slice() {
            // Skip whitespace between tokens.
            while pos < source.len() {
                let ch = match source[pos..].chars().next() {
                    Some(c) => c,
                    None => break,
                };
                if ch.is_whitespace() {
                    pos += ch.len_utf8();
                } else {
                    break;
                }
            }
            let start = pos;
            pos += token.text.len();
            result.push(SpannedToken {
                kind: token.kind.clone(),
                text: token.text.clone(),
                span: Span { start, end: pos },
            });
        }
        Ok(result)
    }

    /// Parse `input` into an [`Ast`] using the Pratt parser.
    ///
    /// This lexes the input and then parses the token stream.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on lex or parse failure.
    pub fn parse(&self, input: &str) -> Result<Ast, LangError> {
        let tokens = self.lex(input)?;
        self.runtime.parse(&tokens)
    }

    /// Parse `input` as a module (multiple top-level statements) into an
    /// [`Ast`].
    ///
    /// In the data-driven model, the Pratt parser handles both single
    /// expressions and multi-statement modules in a single `parse` call;
    /// the resulting [`Ast`] arena contains all top-level nodes.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on lex or parse failure.
    pub fn parse_module(&self, input: &str) -> Result<Ast, LangError> {
        self.parse(input)
    }

    /// Evaluate `input` into a [`Value`] using the tree-walk evaluator.
    ///
    /// This lexes, parses, and evaluates the input.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on any stage failure.
    pub fn eval(&self, input: &str) -> Result<Value, LangError> {
        let ast = self.parse(input)?;
        self.runtime.eval(&ast)
    }

    /// Convenience: `eval(parse(lex(input)?)?)`.
    ///
    /// # Errors
    ///
    /// Propagates the first stage error.
    pub fn run(&self, input: &str) -> Result<Value, LangError> {
        self.runtime.run(&Source::new(input))
    }

    /// Returns the scope name for this kit's grammar.
    pub fn scope_name(&self) -> &str {
        "source.handl"
    }

    /// Returns the file extensions for this kit's grammar.
    pub fn file_extensions(&self) -> Vec<String> {
        vec!["hnl".to_string()]
    }

    /// Returns the underlying [`Runtime`], giving callers direct access to
    /// the staged `lex`/`parse`/`eval` methods with [`Source`]/[`Tokens`]/
    /// [`Ast`] inputs.
    pub fn runtime(&self) -> &Runtime<'static> {
        &self.runtime
    }
}

impl Default for Kit {
    fn default() -> Self {
        Self::new().expect("canonical kit assembly should not fail")
    }
}

// ---------------------------------------------------------------------------
// Platform AstNode conversion helpers
// ---------------------------------------------------------------------------

/// Convert a [`handl_language::AstNode`] to a [`handl_platform::AstNode`].
///
/// The two types are structurally identical (`kind`, `span`, `payload`) but
/// defined in different crates. This helper bridges them so hosts that
/// dispatch evaluation through the platform plugin system (e.g. `host-cli`)
/// can consume the data-driven parser's output.
pub fn to_platform_ast_node(node: &handl_language::AstNode) -> handl_platform::AstNode {
    handl_platform::AstNode {
        kind: node.kind.clone(),
        span: node.span.map(|s| handl_platform::Span {
            start: s.start,
            end: s.end,
        }),
        payload: node.payload.clone(),
    }
}

/// Convert all nodes in an [`Ast`] arena to a `Vec<handl_platform::AstNode>`.
pub fn ast_to_platform_nodes(ast: &Ast) -> Vec<handl_platform::AstNode> {
    ast.nodes().iter().map(to_platform_ast_node).collect()
}

// ---------------------------------------------------------------------------
// Token classification helpers (string-based kind matching)
// ---------------------------------------------------------------------------

/// Whether a token kind string represents an identifier.
pub fn is_identifier_kind(kind: &str) -> bool {
    kind == "token.identifier"
}

/// Whether a token kind string represents a keyword.
pub fn is_keyword_kind(kind: &str) -> bool {
    kind.starts_with("token.keyword")
}

/// Whether a token kind string represents a number literal.
pub fn is_number_kind(kind: &str) -> bool {
    kind == "token.number"
}

/// Whether a token kind string represents a string literal.
pub fn is_string_kind(kind: &str) -> bool {
    kind == "token.string"
}

/// Whether a token kind string represents a boolean literal.
pub fn is_bool_kind(kind: &str) -> bool {
    kind == "token.bool"
}

/// Whether a token kind string represents the null literal.
pub fn is_null_kind(kind: &str) -> bool {
    kind == "token.null"
}

/// Whether a token kind string represents a comment.
pub fn is_comment_kind(kind: &str) -> bool {
    kind == "token.comment" || kind == "token.doc_comment"
}

/// Whether a token kind string represents a doc comment.
pub fn is_doc_comment_kind(kind: &str) -> bool {
    kind == "token.doc_comment"
}

/// Whether a token kind string represents an operator.
pub fn is_operator_kind(kind: &str) -> bool {
    matches!(
        kind,
        "token.equal"
            | "token.plus"
            | "token.minus"
            | "token.star"
            | "token.pow"
            | "token.slash"
            | "token.lt"
            | "token.gt"
            | "token.eqeq"
            | "token.neq"
            | "token.lte"
            | "token.gte"
            | "token.spaceship"
            | "token.flow"
            | "token.fanout"
            | "token.pipe"
            | "token.arrow"
    )
}

/// Whether a token kind string represents a delimiter/punctuation.
pub fn is_delimiter_kind(kind: &str) -> bool {
    kind.starts_with("token.lparen")
        || kind.starts_with("token.rparen")
        || kind.starts_with("token.lbracket")
        || kind.starts_with("token.rbracket")
        || kind.starts_with("token.lbrace")
        || kind.starts_with("token.rbrace")
        || kind == "token.comma"
        || kind == "token.dot"
        || kind == "token.colon"
        || kind == "token.semicolon"
        || kind == "token.arrow"
}

/// Whether a token kind string represents a template string.
pub fn is_template_kind(kind: &str) -> bool {
    kind == "token.template"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_number() {
        let kit = Kit::new().unwrap();
        let tokens = kit.lex("42").unwrap();
        assert_eq!(tokens.as_slice().len(), 1);
        assert_eq!(tokens.as_slice()[0].kind, "token.number");
        assert_eq!(tokens.as_slice()[0].text, "42");
    }

    #[test]
    fn lex_string() {
        let kit = Kit::new().unwrap();
        let tokens = kit.lex("\"hi\"").unwrap();
        assert_eq!(tokens.as_slice()[0].kind, "token.string");
        assert_eq!(tokens.as_slice()[0].text, "\"hi\"");
    }

    #[test]
    fn lex_identifier() {
        let kit = Kit::new().unwrap();
        let tokens = kit.lex("foo").unwrap();
        assert_eq!(tokens.as_slice()[0].kind, "token.identifier");
        assert_eq!(tokens.as_slice()[0].text, "foo");
    }

    #[test]
    fn lex_with_spans() {
        let kit = Kit::new().unwrap();
        let tokens = kit.lex_with_spans("1 + 2").unwrap();
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, "token.number");
        assert_eq!(tokens[0].text, "1");
        assert_eq!(tokens[0].span.start, 0);
        assert_eq!(tokens[0].span.end, 1);
        assert_eq!(tokens[1].kind, "token.plus");
        assert_eq!(tokens[1].span.start, 2);
        assert_eq!(tokens[1].span.end, 3);
        assert_eq!(tokens[2].kind, "token.number");
        assert_eq!(tokens[2].text, "2");
        assert_eq!(tokens[2].span.start, 4);
        assert_eq!(tokens[2].span.end, 5);
    }

    #[test]
    fn parse_number() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse("1").unwrap();
        assert!(ast.len() > 0);
    }

    #[test]
    fn parse_call() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse("add(1, 2)").unwrap();
        assert!(ast.len() > 0);
    }

    #[test]
    fn parse_index() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse("arr[0]").unwrap();
        assert!(ast.len() > 0);
    }

    #[test]
    fn parse_let() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse("a = 1").unwrap();
        assert!(ast.len() > 0);
    }

    #[test]
    fn parse_string_literal() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse("\"hi\"").unwrap();
        assert!(ast.len() > 0);
    }

    #[test]
    fn parse_bool_literal() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse("true").unwrap();
        assert!(ast.len() > 0);
    }

    #[test]
    fn parse_null_literal() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse("null").unwrap();
        assert!(ast.len() > 0);
    }

    #[test]
    fn parse_array() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse("[1, 2]").unwrap();
        assert!(ast.len() > 0);
    }

    #[test]
    fn parse_object() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse("{a: 1}").unwrap();
        assert!(ast.len() > 0);
    }

    #[test]
    fn parse_match() {
        // The Pratt engine's grammar row for `match` is a prefix compound
        // with arity 2 (scrutinee + arms). `match x` provides only the
        // scrutinee, so the parser errors on the missing arms. This is
        // expected: the full `match x { ... }` syntax requires a grammar
        // engine that supports delimited, variable-arity forms.
        let kit = Kit::new().unwrap();
        let result = kit.parse("match x");
        assert!(result.is_err(), "expected parse error for incomplete match");
    }

    #[test]
    fn parse_handle() {
        // Similar to match: `handle` is a prefix compound that expects
        // additional operands. `handle x` alone is incomplete.
        let kit = Kit::new().unwrap();
        let result = kit.parse("handle x");
        assert!(result.is_err(), "expected parse error for incomplete handle");
    }

    #[test]
    fn parse_module_empty() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse_module("").unwrap();
        assert_eq!(ast.len(), 0);
    }

    #[test]
    fn parse_module_single_let() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse_module("a = 1").unwrap();
        assert!(ast.len() >= 1);
    }

    #[test]
    fn parse_module_multiple_let() {
        let kit = Kit::new().unwrap();
        let ast = kit.parse_module("a = 1; b = 2").unwrap();
        assert!(ast.len() >= 2);
    }

    #[test]
    fn eval_number() {
        let kit = Kit::new().unwrap();
        let val = kit.eval("42").unwrap();
        assert_eq!(val, serde_json::json!(42));
    }

    #[test]
    fn eval_string() {
        let kit = Kit::new().unwrap();
        let val = kit.eval("\"hi\"").unwrap();
        assert_eq!(val, serde_json::json!("hi"));
    }

    #[test]
    fn eval_bool() {
        let kit = Kit::new().unwrap();
        let val = kit.eval("true").unwrap();
        assert_eq!(val, serde_json::json!(true));
    }

    #[test]
    fn eval_null() {
        let kit = Kit::new().unwrap();
        let val = kit.eval("null").unwrap();
        assert_eq!(val, serde_json::Value::Null);
    }

    #[test]
    fn eval_arith() {
        let kit = Kit::new().unwrap();
        let val = kit.eval("1 + 2").unwrap();
        assert_eq!(val, serde_json::json!(3));
    }

    #[test]
    fn run_full_pipeline() {
        let kit = Kit::new().unwrap();
        let val = kit.run("1 + 2 * 3").unwrap();
        assert_eq!(val, serde_json::json!(7));
    }

    #[test]
    fn from_manifest_works() {
        let manifest = canonical_manifest();
        let kit = Kit::from_manifest(&manifest).unwrap();
        let tokens = kit.lex("42").unwrap();
        assert_eq!(tokens.as_slice()[0].kind, "token.number");
    }


    #[test]
    fn scope_name_and_extensions() {
        let kit = Kit::new().unwrap();
        assert_eq!(kit.scope_name(), "source.handl");
        assert_eq!(kit.file_extensions(), vec!["hnl".to_string()]);
    }

    #[test]
    fn to_platform_ast_node_converts() {
        let node = handl_language::AstNode::new("value")
            .with_span(handl_language::Span { start: 0, end: 2 })
            .with_payload(serde_json::json!({ "value": 42 }));
        let platform_node = to_platform_ast_node(&node);
        assert_eq!(platform_node.kind, "value");
        assert_eq!(platform_node.span, Some(handl_platform::Span { start: 0, end: 2 }));
        assert_eq!(platform_node.payload, serde_json::json!({ "value": 42 }));
    }

    // --- Token classification helpers ---

    #[test]
    fn is_identifier_kind_works() {
        assert!(is_identifier_kind("token.identifier"));
        assert!(!is_identifier_kind("token.number"));
    }

    #[test]
    fn is_keyword_kind_works() {
        assert!(is_keyword_kind("token.keyword.let"));
        assert!(is_keyword_kind("token.keyword.match"));
        assert!(!is_keyword_kind("token.identifier"));
    }

    #[test]
    fn is_number_kind_works() {
        assert!(is_number_kind("token.number"));
        assert!(!is_number_kind("token.string"));
    }
}
