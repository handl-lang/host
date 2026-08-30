//! # handl-emitter-tokens
//!
//! Foundation-layer emitter for the entire handl language's **token kind
//! definitions** and **AST node shape definitions** (ADR-0044 Phase 4).
//!
//! This is the data foundation that the grammar and eval emitters
//! (`handl-emitter-arith`, `handl-emitter-let`, `handl-emitter-branch`, …)
//! reference. It consolidates the token kinds currently spread across the
//! `plugin-token-*` crates and the AST node shapes currently spread across the
//! `plugin-ast-*` crates into a single set of [`Language`] data fragments.
//!
//! ## Token kinds
//!
//! Every token kind in the canonical handl dialect is declared as a
//! [`TokenKindDef`] with a stable id (e.g. `token.plus`, `token.lparen`,
//! `token.keyword.let`). The full set covers:
//!
//! - **Operators** (from `plugin-token-operator`): `|>`, `->`, `=`, `+`, `-`,
//!   `*`, `**`, `/`, `<`, `>`, `==`, `!=`, `<=`, `>=`, `<=>`, `>>>`, `&&&`.
//! - **Delimiters** (from `plugin-token-delimiter`): `(`, `)`, `[`, `]`, `{`,
//!   `}`, `,`, `;`, `:`, `.`.
//! - **Literals** (from `plugin-token-bool`, `plugin-token-null`,
//!   `plugin-token-number`, `plugin-token-string`): `bool`, `null`, `number`,
//!   `string`.
//! - **Identifiers** (from `plugin-token-identifier`): `identifier`.
//! - **Keywords** (from `plugin-token-keyword`, `plugin-token-keyword-as`,
//!   `plugin-token-keyword-handle`): `let`, `const`, `if`, `else`, `match`,
//!   `handle`, `as`, `import`, `export`, `module`, `typeof`, `not`.
//! - **Comments** (from `plugin-token-comment`, `plugin-token-doc-comment`):
//!   `comment`, `doc_comment`.
//! - **EOF**: `eof`.
//!
//! ## AST shapes
//!
//! Every AST node kind in the canonical handl dialect is declared as an
//! [`AstShapeDef`] with a stable id (e.g. `ast.var`, `ast.binary.add`) and a
//! `node_kind` matching the kind string the engines use (e.g. `"var"`,
//! `"binary.add"`). Each carries a simple JSON Schema fragment describing its
//! payload shape — these are documentation/validation hints, not enforced at
//! runtime yet.
//!
//! ## Lexer rules
//!
//! In addition to token kinds and AST shapes, this emitter declares the
//! **lexer rule patterns** ([`LexerRuleDef`]) that a data-driven lexer engine
//! (e.g. `plugin-engine-lex-pattern`) reads to tokenize source text. Each rule
//! maps a token kind to a [`LexerPattern`] (literal, character class, number,
//! or delimited block) plus a priority — lower priority = tried first. This
//! makes the lexer fully data-driven: the Rust `plugin-lexer-*` crates can be
//! replaced by the data-driven engine reading these fragments.

#![warn(missing_docs)]

use handl_emitters::Emitter;
use handl_language::{
    AstShapeDef, CapabilityDecl, LangError, Language, LexerPattern, LexerRuleDef, TokenKindDef,
    Value,
};
use serde_json::json;

/// Emitter for the foundation token-kind and AST-shape layer.
#[derive(Debug, Default, Clone, Copy)]
pub struct TokensEmitter;

impl TokensEmitter {
    /// Create a new tokens emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for TokensEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        emit()
    }
}

/// Emit the foundation layer's [`Language`] fragments: all token kind
/// definitions, all AST node shape definitions, and the lexer capability
/// declaration.
///
/// # Errors
///
/// Returns [`LangError`] if any fragment merge fails (collision). All
/// fragments emitted here are unique by construction, so this only fails if
/// the schema itself is inconsistent.
pub fn emit() -> Result<Language, LangError> {
    let mut lang = Language::new();
    declare_tokens(&mut lang)?;
    declare_ast_shapes(&mut lang)?;
    declare_lexer_rules(&mut lang)?;
    declare_capabilities(&mut lang)?;
    Ok(lang)
}

// ---------------------------------------------------------------------------
// Token kinds
// ---------------------------------------------------------------------------

/// Declare every token kind in the canonical handl dialect.
fn declare_tokens(lang: &mut Language) -> Result<(), LangError> {
    // --- Operators (plugin-token-operator) ---
    declare_token_with_label(lang, "token.pipe", "|>")?;
    declare_token_with_label(lang, "token.arrow", "->")?;
    declare_token_with_label(lang, "token.equal", "=")?;
    declare_token_with_label(lang, "token.plus", "+")?;
    declare_token_with_label(lang, "token.minus", "-")?;
    declare_token_with_label(lang, "token.star", "*")?;
    declare_token_with_label(lang, "token.pow", "**")?;
    declare_token_with_label(lang, "token.slash", "/")?;
    declare_token_with_label(lang, "token.lt", "<")?;
    declare_token_with_label(lang, "token.gt", ">")?;
    declare_token_with_label(lang, "token.eqeq", "==")?;
    declare_token_with_label(lang, "token.neq", "!=")?;
    declare_token_with_label(lang, "token.lte", "<=")?;
    declare_token_with_label(lang, "token.gte", ">=")?;
    declare_token_with_label(lang, "token.spaceship", "<=>")?;
    declare_token_with_label(lang, "token.flow", ">>>")?;
    declare_token_with_label(lang, "token.fanout", "&&&")?;

    // --- Delimiters (plugin-token-delimiter) ---
    declare_token_with_label(lang, "token.lparen", "(")?;
    declare_token_with_label(lang, "token.rparen", ")")?;
    declare_token_with_label(lang, "token.lbracket", "[")?;
    declare_token_with_label(lang, "token.rbracket", "]")?;
    declare_token_with_label(lang, "token.lbrace", "{")?;
    declare_token_with_label(lang, "token.rbrace", "}")?;
    declare_token_with_label(lang, "token.comma", ",")?;
    declare_token_with_label(lang, "token.semicolon", ";")?;
    declare_token_with_label(lang, "token.colon", ":")?;
    declare_token_with_label(lang, "token.dot", ".")?;

    // --- Literals (plugin-token-bool, null, number, string) ---
    declare_token_with_label(lang, "token.bool", "bool")?;
    declare_token_with_label(lang, "token.null", "null")?;
    declare_token_with_label(lang, "token.number", "number")?;
    declare_token_with_label(lang, "token.string", "string")?;

    // --- Identifiers (plugin-token-identifier) ---
    declare_token_with_label(lang, "token.identifier", "identifier")?;

    // --- Keywords (plugin-token-keyword, keyword-as, keyword-handle) ---
    declare_token_with_label(lang, "token.keyword.let", "let")?;
    declare_token_with_label(lang, "token.keyword.const", "const")?;
    declare_token_with_label(lang, "token.keyword.if", "if")?;
    declare_token_with_label(lang, "token.keyword.else", "else")?;
    declare_token_with_label(lang, "token.keyword.match", "match")?;
    declare_token_with_label(lang, "token.keyword.handle", "handle")?;
    declare_token_with_label(lang, "token.keyword.as", "as")?;
    declare_token_with_label(lang, "token.keyword.import", "import")?;
    declare_token_with_label(lang, "token.keyword.export", "export")?;
    declare_token_with_label(lang, "token.keyword.module", "module")?;
    declare_token_with_label(lang, "token.keyword.typeof", "typeof")?;
    declare_token_with_label(lang, "token.keyword.not", "not")?;

    // --- Comments (plugin-token-comment, doc-comment) ---
    declare_token_with_label(lang, "token.comment", "comment")?;
    declare_token_with_label(lang, "token.doc_comment", "doc_comment")?;

    // --- EOF ---
    declare_token_with_label(lang, "token.eof", "eof")?;

    Ok(())
}

/// Declare a token kind with a human-readable label.
///
/// Convenience wrapper around [`TokenKindDef::new`] + `with_label` + merge.
fn declare_token_with_label(
    lang: &mut Language,
    id: &str,
    label: &str,
) -> Result<(), LangError> {
    lang.tokens
        .merge(TokenKindDef::new(id).with_label(label))
}

// ---------------------------------------------------------------------------
// AST shapes
// ---------------------------------------------------------------------------

/// Declare every AST node shape in the canonical handl dialect.
fn declare_ast_shapes(lang: &mut Language) -> Result<(), LangError> {
    // --- Primary atoms ---
    declare_shape(
        lang,
        "ast.var",
        "var",
        json!({"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]}),
    )?;
    declare_shape(
        lang,
        "ast.value",
        "value",
        json!({"type": "object", "properties": {"value": {}}, "required": ["value"]}),
    )?;

    // --- Compound expressions ---
    declare_shape(
        lang,
        "ast.call",
        "call",
        json!({
            "type": "object",
            "properties": {
                "callee": {"type": "object"},
                "args": {"type": "array", "items": {"type": "object"}}
            },
            "required": ["callee", "args"]
        }),
    )?;
    declare_shape(
        lang,
        "ast.let",
        "let",
        json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "value": {"type": "object"}
            },
            "required": ["name", "value"]
        }),
    )?;
    declare_shape(
        lang,
        "ast.const",
        "const",
        json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "value": {"type": "object"}
            },
            "required": ["name", "value"]
        }),
    )?;
    declare_shape(
        lang,
        "ast.array",
        "array",
        json!({
            "type": "object",
            "properties": {
                "elements": {"type": "array", "items": {"type": "object"}}
            },
            "required": ["elements"]
        }),
    )?;
    declare_shape(
        lang,
        "ast.object",
        "object",
        json!({
            "type": "object",
            "properties": {
                "entries": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "key": {"type": "string"},
                            "value": {"type": "object"}
                        },
                        "required": ["key", "value"]
                    }
                }
            },
            "required": ["entries"]
        }),
    )?;
    declare_shape(
        lang,
        "ast.index",
        "index",
        json!({
            "type": "object",
            "properties": {
                "target": {"type": "object"},
                "index": {"type": "object"}
            },
            "required": ["target", "index"]
        }),
    )?;
    declare_shape(
        lang,
        "ast.field-access",
        "field-access",
        json!({
            "type": "object",
            "properties": {
                "target": {"type": "object"},
                "field": {"type": "string"}
            },
            "required": ["target", "field"]
        }),
    )?;
    declare_shape(
        lang,
        "ast.match",
        "match",
        json!({
            "type": "object",
            "properties": {
                "scrutinee": {"type": "object"},
                "arms": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "pattern": {"type": "object"},
                            "body": {"type": "object"}
                        },
                        "required": ["pattern", "body"]
                    }
                }
            },
            "required": ["scrutinee", "arms"]
        }),
    )?;
    declare_shape(
        lang,
        "ast.handle",
        "handle",
        json!({
            "type": "object",
            "properties": {
                "body": {"type": "object"},
                "clauses": {
                    "type": "array",
                    "items": {"type": "object"}
                }
            },
            "required": ["body", "clauses"]
        }),
    )?;
    declare_shape(
        lang,
        "ast.pipe",
        "pipe",
        json!({
            "type": "object",
            "properties": {
                "stages": {"type": "array", "items": {"type": "object"}}
            },
            "required": ["stages"]
        }),
    )?;
    declare_shape(
        lang,
        "ast.assign",
        "assign",
        json!({
            "type": "object",
            "properties": {
                "target": {"type": "object"},
                "value": {"type": "object"}
            },
            "required": ["target", "value"]
        }),
    )?;
    declare_shape(
        lang,
        "ast.apply",
        "apply",
        json!({
            "type": "object",
            "properties": {
                "callee": {"type": "object"},
                "args": {"type": "array", "items": {"type": "object"}}
            },
            "required": ["callee", "args"]
        }),
    )?;
    declare_shape(
        lang,
        "ast.function",
        "function",
        json!({
            "type": "object",
            "properties": {
                "params": {"type": "array", "items": {"type": "string"}},
                "body": {"type": "object"}
            },
            "required": ["params", "body"]
        }),
    )?;

    // --- Binary arithmetic nodes (children: [idx, idx]) ---
    declare_binary_shape(lang, "ast.binary.add", "binary.add")?;
    declare_binary_shape(lang, "ast.binary.sub", "binary.sub")?;
    declare_binary_shape(lang, "ast.binary.mul", "binary.mul")?;
    declare_binary_shape(lang, "ast.binary.div", "binary.div")?;
    declare_binary_shape(lang, "ast.binary.pow", "binary.pow")?;

    // --- Binary comparison nodes (children: [idx, idx]) ---
    declare_binary_shape(lang, "ast.binary.eq", "binary.eq")?;
    declare_binary_shape(lang, "ast.binary.neq", "binary.neq")?;
    declare_binary_shape(lang, "ast.binary.lt", "binary.lt")?;
    declare_binary_shape(lang, "ast.binary.lte", "binary.lte")?;
    declare_binary_shape(lang, "ast.binary.gt", "binary.gt")?;
    declare_binary_shape(lang, "ast.binary.gte", "binary.gte")?;
    declare_binary_shape(lang, "ast.binary.compare", "binary.compare")?;

    // --- Unary nodes (children: [idx]) ---
    declare_shape(
        lang,
        "ast.unary.not",
        "unary.not",
        json!({
            "type": "object",
            "properties": {
                "children": {"type": "array", "items": {"type": "integer"}, "minItems": 1, "maxItems": 1}
            },
            "required": ["children"]
        }),
    )?;

    // --- Control-flow / structural nodes ---
    declare_shape(
        lang,
        "ast.if",
        "if",
        json!({
            "type": "object",
            "properties": {
                "children": {
                    "type": "array",
                    "items": {"type": "integer"},
                    "minItems": 3,
                    "maxItems": 3
                }
            },
            "required": ["children"]
        }),
    )?;
    declare_shape(
        lang,
        "ast.paren",
        "paren",
        json!({
            "type": "object",
            "properties": {
                "children": {"type": "array", "items": {"type": "integer"}, "minItems": 1, "maxItems": 1}
            },
            "required": ["children"]
        }),
    )?;
    declare_shape(
        lang,
        "ast.typeof",
        "typeof",
        json!({
            "type": "object",
            "properties": {
                "children": {"type": "array", "items": {"type": "integer"}, "minItems": 1, "maxItems": 1}
            },
            "required": ["children"]
        }),
    )?;

    // --- Module-system nodes ---
    declare_shape(
        lang,
        "ast.import",
        "import",
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "items": {"type": "array", "items": {"type": "string"}}
            },
            "required": ["path"]
        }),
    )?;
    declare_shape(
        lang,
        "ast.export",
        "export",
        json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "value": {"type": "object"}
            },
            "required": ["name", "value"]
        }),
    )?;
    declare_shape(
        lang,
        "ast.module",
        "module",
        json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "body": {"type": "array", "items": {"type": "object"}}
            },
            "required": ["name", "body"]
        }),
    )?;

    // --- Template literals ---
    declare_shape(
        lang,
        "ast.template",
        "template",
        json!({
            "type": "object",
            "properties": {
                "segments": {"type": "array", "items": {"type": "object"}}
            },
            "required": ["segments"]
        }),
    )?;

    // --- Type annotations ---
    declare_shape(
        lang,
        "ast.type-annot",
        "type-annot",
        json!({
            "type": "object",
            "properties": {
                "type": {"type": "string"}
            },
            "required": ["type"]
        }),
    )?;

    Ok(())
}

/// Declare an AST shape with the given id, node kind, and payload schema.
fn declare_shape(
    lang: &mut Language,
    id: &str,
    node_kind: &str,
    schema: Value,
) -> Result<(), LangError> {
    lang.ast
        .merge(AstShapeDef::new(id, node_kind).with_payload_schema(schema))
}

/// Declare a binary AST shape whose payload is `{"children": [idx, idx]}`.
fn declare_binary_shape(
    lang: &mut Language,
    id: &str,
    node_kind: &str,
) -> Result<(), LangError> {
    declare_shape(
        lang,
        id,
        node_kind,
        json!({
            "type": "object",
            "properties": {
                "children": {
                    "type": "array",
                    "items": {"type": "integer"},
                    "minItems": 2,
                    "maxItems": 2
                }
            },
            "required": ["children"]
        }),
    )
}

// ---------------------------------------------------------------------------
// Capabilities
// ---------------------------------------------------------------------------

/// Declare the capabilities this foundation layer exercises.
fn declare_capabilities(lang: &mut Language) -> Result<(), LangError> {
    lang.caps.merge(
        CapabilityDecl::new("lex")
            .with_description("lex source text into tokens"),
    )?;
    lang.caps.merge(
        CapabilityDecl::new("parse")
            .with_description("parse tokens into an AST"),
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Lexer rules
// ---------------------------------------------------------------------------

/// Declare a lexer rule that matches a literal string.
fn lex_literal(
    lang: &mut Language,
    id: &str,
    token_kind: &str,
    text: &str,
    priority: i32,
) -> Result<(), LangError> {
    lang.lexer_rules.merge(LexerRuleDef::new(
        id,
        token_kind,
        LexerPattern::Literal { text: text.into() },
        priority,
    ))
}

/// Declare a lexer rule that matches a delimited block.
fn lex_delimited(
    lang: &mut Language,
    id: &str,
    token_kind: &str,
    open: &str,
    close: &str,
    escape: bool,
    interpolation: bool,
    close_at_eof: bool,
    priority: i32,
) -> Result<(), LangError> {
    lang.lexer_rules.merge(LexerRuleDef::new(
        id,
        token_kind,
        LexerPattern::Delimited {
            open: open.into(),
            close: close.into(),
            escape,
            interpolation,
            close_at_eof,
        },
        priority,
    ))
}

/// Declare every lexer rule in the canonical handl dialect.
///
/// Priority ordering (lower = tried first):
/// 1. Comments (priority 3-5) — tried before everything.
/// 2. Multi-char operators (priority 5-10) — tried before single-char.
/// 3. Keywords/bool/null (priority 15) — tried before identifiers.
/// 4. Single-char operators/delimiters (priority 20-30).
/// 5. Numbers/strings/templates (priority 40).
/// 6. Identifiers (priority 50) — tried last.
fn declare_lexer_rules(lang: &mut Language) -> Result<(), LangError> {
    // --- Comments (tried before everything) ---
    // Doc comment (`///`) is tried before line comment (`//`) so `///` is not
    // consumed as a `//` comment.
    lex_delimited(
        lang,
        "lex.doc_comment",
        "token.doc_comment",
        "///",
        "\n",
        false,
        false,
        true,
        3,
    )?;
    lex_delimited(
        lang,
        "lex.comment",
        "token.comment",
        "//",
        "\n",
        false,
        false,
        true,
        5,
    )?;

    // --- Multi-char operators (tried before single-char) ---
    lex_literal(lang, "lex.spaceship", "token.spaceship", "<=>", 5)?;
    lex_literal(lang, "lex.pipe", "token.pipe", "|>", 10)?;
    lex_literal(lang, "lex.arrow", "token.arrow", "->", 10)?;
    lex_literal(lang, "lex.flow", "token.flow", ">>>", 10)?;
    lex_literal(lang, "lex.fanout", "token.fanout", "&&&", 10)?;
    lex_literal(lang, "lex.eqeq", "token.eqeq", "==", 10)?;
    lex_literal(lang, "lex.neq", "token.neq", "!=", 10)?;
    lex_literal(lang, "lex.lte", "token.lte", "<=", 10)?;
    lex_literal(lang, "lex.gte", "token.gte", ">=", 10)?;
    lex_literal(lang, "lex.pow", "token.pow", "**", 10)?;

    // --- Keywords / bool / null (tried before identifiers) ---
    lex_literal(lang, "lex.bool.true", "token.bool", "true", 15)?;
    lex_literal(lang, "lex.bool.false", "token.bool", "false", 15)?;
    lex_literal(lang, "lex.null", "token.null", "null", 15)?;
    lex_literal(lang, "lex.keyword.let", "token.keyword.let", "let", 15)?;
    lex_literal(lang, "lex.keyword.const", "token.keyword.const", "const", 15)?;
    lex_literal(lang, "lex.keyword.if", "token.keyword.if", "if", 15)?;
    lex_literal(lang, "lex.keyword.else", "token.keyword.else", "else", 15)?;
    lex_literal(lang, "lex.keyword.match", "token.keyword.match", "match", 15)?;
    lex_literal(lang, "lex.keyword.handle", "token.keyword.handle", "handle", 15)?;
    lex_literal(lang, "lex.keyword.as", "token.keyword.as", "as", 15)?;
    lex_literal(lang, "lex.keyword.import", "token.keyword.import", "import", 15)?;
    lex_literal(lang, "lex.keyword.export", "token.keyword.export", "export", 15)?;
    lex_literal(lang, "lex.keyword.module", "token.keyword.module", "module", 15)?;
    lex_literal(lang, "lex.keyword.typeof", "token.keyword.typeof", "typeof", 15)?;
    lex_literal(lang, "lex.keyword.not", "token.keyword.not", "not", 15)?;

    // --- Single-char operators ---
    lex_literal(lang, "lex.equal", "token.equal", "=", 20)?;
    lex_literal(lang, "lex.plus", "token.plus", "+", 20)?;
    lex_literal(lang, "lex.minus", "token.minus", "-", 20)?;
    lex_literal(lang, "lex.star", "token.star", "*", 20)?;
    lex_literal(lang, "lex.slash", "token.slash", "/", 20)?;
    lex_literal(lang, "lex.lt", "token.lt", "<", 20)?;
    lex_literal(lang, "lex.gt", "token.gt", ">", 20)?;

    // --- Delimiters ---
    lex_literal(lang, "lex.lparen", "token.lparen", "(", 30)?;
    lex_literal(lang, "lex.rparen", "token.rparen", ")", 30)?;
    lex_literal(lang, "lex.lbracket", "token.lbracket", "[", 30)?;
    lex_literal(lang, "lex.rbracket", "token.rbracket", "]", 30)?;
    lex_literal(lang, "lex.lbrace", "token.lbrace", "{", 30)?;
    lex_literal(lang, "lex.rbrace", "token.rbrace", "}", 30)?;
    lex_literal(lang, "lex.comma", "token.comma", ",", 30)?;
    lex_literal(lang, "lex.semicolon", "token.semicolon", ";", 30)?;
    lex_literal(lang, "lex.colon", "token.colon", ":", 30)?;
    lex_literal(lang, "lex.dot", "token.dot", ".", 30)?;

    // --- Literals: number, string, template ---
    lang.lexer_rules.merge(LexerRuleDef::new(
        "lex.number",
        "token.number",
        LexerPattern::Number,
        40,
    ))?;
    lex_delimited(
        lang,
        "lex.string",
        "token.string",
        "\"",
        "\"",
        true,
        false,
        false,
        40,
    )?;
    lex_delimited(
        lang,
        "lex.template",
        "token.template",
        "`",
        "`",
        true,
        true,
        false,
        40,
    )?;

    // --- Identifier (tried last — catches anything that's not a keyword) ---
    lang.lexer_rules.merge(LexerRuleDef::new(
        "lex.identifier",
        "token.identifier",
        LexerPattern::CharClass {
            start_chars: "a-zA-Z_".into(),
            continue_chars: "a-zA-Z0-9_".into(),
        },
        50,
    ))?;

    // Note: `token.eof` has no lexer rule — the data-driven lexer engine
    // produces EOF automatically (it stops at end of input).

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Total number of token kinds emitted.
    const EXPECTED_TOKEN_COUNT: usize = 47;
    /// Total number of AST shapes emitted.
    const EXPECTED_AST_COUNT: usize = 36;
    /// Total number of lexer rules emitted.
    const EXPECTED_LEXER_RULE_COUNT: usize = 48;

    #[test]
    fn emit_produces_expected_token_count() {
        let lang = emit().unwrap();
        assert_eq!(
            lang.tokens.len(),
            EXPECTED_TOKEN_COUNT,
            "token kind count mismatch"
        );
    }

    #[test]
    fn emit_produces_expected_ast_count() {
        let lang = emit().unwrap();
        assert_eq!(
            lang.ast.len(),
            EXPECTED_AST_COUNT,
            "AST shape count mismatch"
        );
    }

    #[test]
    fn emit_produces_expected_capability_count() {
        let lang = emit().unwrap();
        assert_eq!(lang.caps.len(), 2);
    }

    #[test]
    fn emit_produces_expected_lexer_rule_count() {
        let lang = emit().unwrap();
        assert_eq!(
            lang.lexer_rules.len(),
            EXPECTED_LEXER_RULE_COUNT,
            "lexer rule count mismatch"
        );
    }

    #[test]
    fn lexer_rule_for_number_exists() {
        let lang = emit().unwrap();
        let rule = lang.lexer_rules.get_by_id("lex.number").unwrap();
        assert_eq!(rule.token_kind, "token.number");
        assert!(matches!(rule.pattern, LexerPattern::Number));
        assert_eq!(rule.priority, 40);
    }

    #[test]
    fn lexer_rule_for_identifier_exists() {
        let lang = emit().unwrap();
        let rule = lang.lexer_rules.get_by_id("lex.identifier").unwrap();
        assert_eq!(rule.token_kind, "token.identifier");
        assert!(matches!(rule.pattern, LexerPattern::CharClass { .. }));
        assert_eq!(rule.priority, 50);
    }

    #[test]
    fn lexer_rule_for_string_is_delimited() {
        let lang = emit().unwrap();
        let rule = lang.lexer_rules.get_by_id("lex.string").unwrap();
        assert_eq!(rule.token_kind, "token.string");
        match &rule.pattern {
            LexerPattern::Delimited {
                open,
                close,
                escape,
                interpolation,
                close_at_eof,
            } => {
                assert_eq!(open, "\"");
                assert_eq!(close, "\"");
                assert!(*escape);
                assert!(!*interpolation);
                assert!(!*close_at_eof);
            }
            other => panic!("expected Delimited, got {other:?}"),
        }
    }

    #[test]
    fn lexer_rules_for_bool_are_two_literals() {
        let lang = emit().unwrap();
        let t = lang.lexer_rules.get_by_id("lex.bool.true").unwrap();
        assert_eq!(t.token_kind, "token.bool");
        let f = lang.lexer_rules.get_by_id("lex.bool.false").unwrap();
        assert_eq!(f.token_kind, "token.bool");
        // Both produce the same token kind but have distinct ids.
        assert_ne!(t.id, f.id);
    }

    #[test]
    fn lexer_rule_doc_comment_tried_before_comment() {
        let lang = emit().unwrap();
        let doc = lang.lexer_rules.get_by_id("lex.doc_comment").unwrap();
        let com = lang.lexer_rules.get_by_id("lex.comment").unwrap();
        assert!(doc.priority < com.priority);
    }

    #[test]
    fn lexer_rule_multi_char_ops_before_single_char() {
        let lang = emit().unwrap();
        let eqeq = lang.lexer_rules.get_by_id("lex.eqeq").unwrap();
        let eq = lang.lexer_rules.get_by_id("lex.equal").unwrap();
        assert!(eqeq.priority < eq.priority);
        let pow = lang.lexer_rules.get_by_id("lex.pow").unwrap();
        let star = lang.lexer_rules.get_by_id("lex.star").unwrap();
        assert!(pow.priority < star.priority);
    }

    #[test]
    fn lexer_rule_keywords_before_identifiers() {
        let lang = emit().unwrap();
        let kw = lang.lexer_rules.get_by_id("lex.keyword.let").unwrap();
        let ident = lang.lexer_rules.get_by_id("lex.identifier").unwrap();
        assert!(kw.priority < ident.priority);
    }

    #[test]
    fn all_lexer_rule_ids_unique() {
        let lang = emit().unwrap();
        let mut ids: Vec<String> = lang
            .lexer_rules
            .iter()
            .map(|r| r.id.clone())
            .collect();
        ids.sort();
        let before = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), before, "duplicate lexer rule ids detected");
    }

    #[test]
    fn emitter_trait_emit_matches_free_function() {
        let e = TokensEmitter::new();
        let a = emit().unwrap();
        let b = e.emit().unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn operator_token_ids_exist() {
        let lang = emit().unwrap();
        for id in [
            "token.pipe",
            "token.arrow",
            "token.equal",
            "token.plus",
            "token.minus",
            "token.star",
            "token.pow",
            "token.slash",
            "token.lt",
            "token.gt",
            "token.eqeq",
            "token.neq",
            "token.lte",
            "token.gte",
            "token.spaceship",
            "token.flow",
            "token.fanout",
        ] {
            assert!(
                lang.tokens.get_by_id(id).is_some(),
                "missing token kind {id}"
            );
        }
    }

    #[test]
    fn delimiter_token_ids_exist() {
        let lang = emit().unwrap();
        for id in [
            "token.lparen",
            "token.rparen",
            "token.lbracket",
            "token.rbracket",
            "token.lbrace",
            "token.rbrace",
            "token.comma",
            "token.semicolon",
            "token.colon",
            "token.dot",
        ] {
            assert!(
                lang.tokens.get_by_id(id).is_some(),
                "missing token kind {id}"
            );
        }
    }

    #[test]
    fn literal_token_ids_exist() {
        let lang = emit().unwrap();
        for id in ["token.bool", "token.null", "token.number", "token.string"] {
            assert!(
                lang.tokens.get_by_id(id).is_some(),
                "missing token kind {id}"
            );
        }
    }

    #[test]
    fn identifier_token_id_exists() {
        let lang = emit().unwrap();
        assert!(lang.tokens.get_by_id("token.identifier").is_some());
    }

    #[test]
    fn keyword_token_ids_exist() {
        let lang = emit().unwrap();
        for id in [
            "token.keyword.let",
            "token.keyword.const",
            "token.keyword.if",
            "token.keyword.else",
            "token.keyword.match",
            "token.keyword.handle",
            "token.keyword.as",
            "token.keyword.import",
            "token.keyword.export",
            "token.keyword.module",
            "token.keyword.typeof",
            "token.keyword.not",
        ] {
            assert!(
                lang.tokens.get_by_id(id).is_some(),
                "missing keyword token kind {id}"
            );
        }
    }

    #[test]
    fn comment_and_eof_token_ids_exist() {
        let lang = emit().unwrap();
        assert!(lang.tokens.get_by_id("token.comment").is_some());
        assert!(lang.tokens.get_by_id("token.doc_comment").is_some());
        assert!(lang.tokens.get_by_id("token.eof").is_some());
    }

    #[test]
    fn token_labels_are_set() {
        let lang = emit().unwrap();
        let plus = lang.tokens.get_by_id("token.plus").unwrap();
        assert_eq!(plus.label.as_deref(), Some("+"));
        let lparen = lang.tokens.get_by_id("token.lparen").unwrap();
        assert_eq!(lparen.label.as_deref(), Some("("));
        let kw = lang.tokens.get_by_id("token.keyword.let").unwrap();
        assert_eq!(kw.label.as_deref(), Some("let"));
    }

    #[test]
    fn core_ast_shapes_exist() {
        let lang = emit().unwrap();
        for kind in [
            "var",
            "value",
            "call",
            "let",
            "const",
            "array",
            "object",
            "index",
            "field-access",
            "match",
            "handle",
            "pipe",
            "assign",
            "apply",
            "function",
        ] {
            assert!(
                lang.ast.get_by_identity(kind).is_some(),
                "missing AST shape for node kind {kind}"
            );
        }
    }

    #[test]
    fn binary_ast_shapes_exist() {
        let lang = emit().unwrap();
        for kind in [
            "binary.add",
            "binary.sub",
            "binary.mul",
            "binary.div",
            "binary.eq",
            "binary.neq",
            "binary.lt",
            "binary.lte",
            "binary.gt",
            "binary.gte",
            "binary.compare",
        ] {
            assert!(
                lang.ast.get_by_identity(kind).is_some(),
                "missing binary AST shape for node kind {kind}"
            );
        }
    }

    #[test]
    fn structural_ast_shapes_exist() {
        let lang = emit().unwrap();
        for kind in [
            "unary.not",
            "if",
            "paren",
            "typeof",
            "import",
            "export",
            "module",
            "template",
            "type-annot",
        ] {
            assert!(
                lang.ast.get_by_identity(kind).is_some(),
                "missing AST shape for node kind {kind}"
            );
        }
    }

    #[test]
    fn ast_shape_payload_schemas_are_set() {
        let lang = emit().unwrap();
        let var = lang.ast.get_by_identity("var").unwrap();
        assert_eq!(var.node_kind, "var");
        assert_eq!(var.payload_schema["type"], "object");
        assert_eq!(var.payload_schema["properties"]["name"]["type"], "string");

        let add = lang.ast.get_by_identity("binary.add").unwrap();
        assert_eq!(add.payload_schema["properties"]["children"]["maxItems"], 2);

        let if_node = lang.ast.get_by_identity("if").unwrap();
        assert_eq!(if_node.payload_schema["properties"]["children"]["maxItems"], 3);
    }

    #[test]
    fn serde_roundtrip() {
        let lang = emit().unwrap();
        let j = serde_json::to_string(&lang).unwrap();
        let back: Language = serde_json::from_str(&j).unwrap();
        assert_eq!(lang, back);
    }

    #[test]
    fn no_collisions_emit_twice_succeeds() {
        // All fragments within a single emit() are unique by construction, so
        // calling emit() twice each succeeds without internal collision errors.
        // (Merging two identical non-union Languages would collide on shared
        // identity keys — that is expected FragmentSet behaviour, not a bug in
        // this emitter. The point of this test is that emit() itself never
        // errors due to duplicate fragments.)
        let a = emit().unwrap();
        let b = emit().unwrap();
        assert_eq!(a.tokens.len(), EXPECTED_TOKEN_COUNT);
        assert_eq!(b.tokens.len(), EXPECTED_TOKEN_COUNT);
        assert_eq!(a, b);
    }

    #[test]
    fn merge_into_empty_succeeds() {
        // Merging one emit() into an empty Language must succeed — this is the
        // real "no collisions" guarantee: every fragment has a unique identity
        // key within the emitted set.
        let mut lang = Language::new();
        lang.merge(emit().unwrap()).unwrap();
        assert_eq!(lang.tokens.len(), EXPECTED_TOKEN_COUNT);
        assert_eq!(lang.ast.len(), EXPECTED_AST_COUNT);
        assert_eq!(lang.caps.len(), 2);
        assert_eq!(lang.lexer_rules.len(), EXPECTED_LEXER_RULE_COUNT);
    }

    #[test]
    fn all_token_ids_unique() {
        let lang = emit().unwrap();
        let mut ids: Vec<String> = lang
            .tokens
            .iter()
            .map(|t| t.id.clone())
            .collect();
        ids.sort();
        let before = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), before, "duplicate token ids detected");
    }

    #[test]
    fn all_ast_node_kinds_unique() {
        let lang = emit().unwrap();
        let mut kinds: Vec<String> = lang
            .ast
            .iter()
            .map(|s| s.node_kind.clone())
            .collect();
        kinds.sort();
        let before = kinds.len();
        kinds.dedup();
        assert_eq!(kinds.len(), before, "duplicate AST node kinds detected");
    }
}
