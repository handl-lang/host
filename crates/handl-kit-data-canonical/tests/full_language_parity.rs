//! Full-language parity test for `handl-kit-data-canonical`.
//!
//! Assembles the complete data-driven canonical kit (all Phase 4 emitters)
//! and verifies the merged [`Language`] covers the full Handl dialect: eval
//! schemes for every major node kind, grammar rows for every major construct,
//! and token kinds for every major token. This is the ADR-0044 Phase 5
//! "full data-driven language" coverage check.

use handl_kit_data_canonical;
use handl_language::Language;

/// Assemble the full canonical kit and return its language.
fn full_language() -> Language {
    let kit = handl_kit_data_canonical::assemble().expect("full kit assembles");
    kit.lang().clone()
}

#[test]
fn full_kit_assembles_without_collisions() {
    // The primary guarantee: merging all Phase 4 emitters (with union
    // semantics for token/AST-shape declarations and excludes for the
    // non-canonical duplicate behavioral fragments) succeeds with no
    // collision errors.
    let _lang = full_language();
}

#[test]
fn language_has_eval_schemes_for_all_major_node_kinds() {
    let lang = full_language();
    let eval_node_kinds: &[&str] = &[
        "var",
        "call",
        "let",
        "if",
        "function",
        "binary.add",
        "binary.sub",
        "binary.mul",
        "binary.div",
        "binary.mod",
        "binary.eq",
        "binary.neq",
        "binary.lt",
        "binary.lte",
        "binary.gt",
        "binary.gte",
        "binary.compare",
        "binary.and",
        "binary.or",
        "array",
        "object",
        "match",
        "handle",
        "index",
        "field-access",
        "pipe",
        "assign",
        "flow",
        "fanout",
        "ternary",
        "unary.not",
        "unary.neg",
        "paren",
        "typeof",
        "template",
        "type-annot",
        "mood",
        "resolve",
        "apply",
        "stmt.let",
        "stmt.const",
        "stmt.import",
        "stmt.export",
        "stmt.module",
        "hello.greet",
        "json",
    ];
    for kind in eval_node_kinds {
        assert!(
            lang.eval.get_by_identity(kind).is_some(),
            "missing eval scheme for node kind {:?}",
            kind,
        );
    }
}

#[test]
fn language_has_grammar_rows_for_all_major_constructs() {
    let lang = full_language();
    let grammar_ids: &[&str] = &[
        "grammar.identifier.prefix",
        "grammar.value.number",
        "grammar.value.string",
        "grammar.value.bool",
        "grammar.value.null",
        "grammar.add.infix",
        "grammar.sub.infix",
        "grammar.mul.infix",
        "grammar.div.infix",
        "grammar.mod.infix",
        "grammar.pow.infix",
        "grammar.and.infix",
        "grammar.or.infix",
        "grammar.let.prefix",
        "grammar.call.infix",
        "grammar.if.prefix",
        "grammar.arrow.infix",
        "grammar.eq.infix",
        "grammar.neq.infix",
        "grammar.lt.infix",
        "grammar.lte.infix",
        "grammar.gt.infix",
        "grammar.gte.infix",
        "grammar.compare.infix",
        "grammar.assign.infix",
        "grammar.pipe.infix",
        "grammar.flow.infix",
        "grammar.fanout.infix",
        "grammar.ternary.infix",
        "grammar.not.prefix",
        "grammar.neg.prefix",
        "grammar.paren.prefix",
        "grammar.typeof.prefix",
        "grammar.array.prefix",
        "grammar.object.prefix",
        "grammar.match.prefix",
        "grammar.handle.prefix",
        "grammar.field-access.postfix",
        "grammar.index.postfix",
        "grammar.mood.postfix",
        "grammar.template.prefix",
        "grammar.type-annot.infix",
        "grammar.stmt-let.prefix",
        "grammar.stmt-const.prefix",
        "grammar.stmt-import.prefix",
        "grammar.stmt-export.prefix",
        "grammar.stmt-module.prefix",
    ];
    for id in grammar_ids {
        assert!(
            lang.grammar.get_by_id(id).is_some(),
            "missing grammar row {:?}",
            id,
        );
    }
}

#[test]
fn language_has_token_kinds_for_all_major_tokens() {
    let lang = full_language();
    let token_ids: &[&str] = &[
        // Operators.
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
        "token.and",
        "token.or",
        "token.percent",
        // Delimiters.
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
        // Literals.
        "token.bool",
        "token.null",
        "token.number",
        "token.string",
        // Identifier.
        "token.identifier",
        // Keywords.
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
        // Tokens declared only by grammar/eval emitters (not the foundation).
        "token.let",
        "token.if",
        "token.bang",
        "token.question",
        "token.template",
        "token.mood",
        // Comments + EOF.
        "token.comment",
        "token.doc_comment",
        "token.eof",
    ];
    for id in token_ids {
        assert!(
            lang.tokens.get_by_id(id).is_some(),
            "missing token kind {:?}",
            id,
        );
    }
}

#[test]
fn language_has_ast_shapes_for_all_major_node_kinds() {
    let lang = full_language();
    let ast_node_kinds: &[&str] = &[
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
        "binary.add",
        "binary.eq",
        "binary.compare",
        "binary.and",
        "binary.or",
        "binary.mod",
        "unary.not",
        "unary.neg",
        "if",
        "paren",
        "typeof",
        "import",
        "export",
        "module",
        "template",
        "type-annot",
        "resolve",
        "hello.greet",
        "json",
    ];
    for kind in ast_node_kinds {
        assert!(
            lang.ast.get_by_identity(kind).is_some(),
            "missing AST shape for node kind {:?}",
            kind,
        );
    }
}

#[test]
fn language_has_core_capabilities() {
    let lang = full_language();
    assert!(lang.caps.get_by_id("lex").is_some());
    assert!(lang.caps.get_by_id("parse").is_some());
    assert!(lang.caps.get_by_id("textmate").is_some());
}

#[test]
fn full_kit_runtime_binds_engines() {
    // The full language must bind to the Pratt parse engine and tree-walk
    // eval engine without error.
    use handl_plugin_engine_eval_tree_walk::TreeWalkEngine;
    use handl_plugin_engine_lex_pattern::DataLexerEngine;
    use handl_plugin_engine_parse_pratt::PrattEngine;
    use handl_runtime::EngineRegistry;
    use std::rc::Rc;
    let kit = handl_kit_data_canonical::assemble().expect("full kit assembles");
    let eval_len = kit.lang().eval.len();
    let grammar_len = kit.lang().grammar.len();
    let mut reg = EngineRegistry::new();
    reg.register_lex("data", Box::new(DataLexerEngine::new()));
    reg.register_parse("pratt", PrattEngine::new());
    reg.register_eval("tree-walk", Box::new(TreeWalkEngine::new()));
    let registry = Rc::new(reg);
    let rt = kit
        .into_runtime(&registry, None)
        .expect("runtime binds engines");
    assert_eq!(rt.lang().eval.len(), eval_len);
    assert_eq!(rt.lang().grammar.len(), grammar_len);
}
