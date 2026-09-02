//! Integration test: assemble `handl-kit-data-canonical` from the YAML data
//! files and verify the resulting `Language` has the expected fragments
//! (ADR-0044 §6 end state, v0.1 surface syntax ADR-0074).

use handl_kit_data_canonical;

#[test]
fn assemble_canonical_has_expected_fragment_counts() {
    let kit = handl_kit_data_canonical::assemble().expect("assembly should succeed");
    let lang = kit.lang();

    // Token kinds: 68 — v0.1 surface syntax (ADR-0074).
    // Removed 17 old tokens (arrow_app, arrow_bind, pipe, returnA, loop,
    // then, where, handle, typeof, module, const, is, may, must, when,
    // not keyword, vau) and added 13 new tokens (rec, while, for, return,
    // law, trait, impl, infer, type, from, product, nullish, opt_chain).
    assert_eq!(lang.tokens.len(), 68);

    // Grammar rows: 56 — v0.1 surface syntax (ADR-0074).
    // Removed 15 old rules (arrow-app, arrow-bind, returnA, loop, pipe,
    // handle, typeof, stmt-const, stmt-module, is, may, must, not.infix,
    // when, vau) and added 12 new rules (product, nullish, opt-chain, rec,
    // while, for, return, law, trait, impl, infer, type).
    assert_eq!(lang.grammar.len(), 56);

    // Eval schemes: 57 — v0.1 surface syntax (ADR-0074).
    // Removed 12 old schemes (arrow_bind, returnA, loop, pipe, handle,
    // typeof, stmt-const, stmt-module, mood.cnd, mood.int, mood.neg,
    // mood.norm, mood.pot) and added 12 new schemes (rec, while, for,
    // return, law, trait, impl, infer, type, product, nullish, opt-chain).
    assert_eq!(lang.eval.len(), 57);

    // AST shapes: 56 — v0.1 surface syntax (ADR-0074).
    // Removed 15 old shapes (arrow_app, arrow_bind, returnA, loop, pipe,
    // handle, typeof, const, module, mood.int, mood.pot, mood.norm,
    // mood.neg, mood.cnd, vau) and added 12 new shapes (rec, while, for,
    // return, law, trait, impl, infer, type, product, nullish, opt-chain).
    assert_eq!(lang.ast.len(), 56);

    // Capabilities: lex + parse (from `tokens`) + textmate (from `textmate`).
    assert_eq!(lang.caps.len(), 3);

    // No options in the canonical kit.
    assert_eq!(lang.options.len(), 0);
}

#[test]
fn assemble_canonical_runtime_config_is_pratt_tree_walk() {
    let kit = handl_kit_data_canonical::assemble().unwrap();
    assert_eq!(kit.runtime_config().parse, "pratt");
    assert_eq!(kit.runtime_config().eval, "tree-walk");
}

#[test]
fn assemble_canonical_into_runtime_binds_engines() {
    use handl_plugin_engine_eval_tree_walk::TreeWalkEngine;
    use handl_plugin_engine_lex_pattern::DataLexerEngine;
    use handl_plugin_engine_parse_pratt::PrattEngine;
    use handl_runtime::EngineRegistry;
    use std::rc::Rc;
    let kit = handl_kit_data_canonical::assemble().unwrap();
    let mut reg = EngineRegistry::new();
    reg.register_lex("data", Box::new(DataLexerEngine::new()));
    reg.register_parse("pratt", PrattEngine::new());
    reg.register_eval("tree-walk", Box::new(TreeWalkEngine::new()));
    let registry = Rc::new(reg);
    let rt = kit
        .into_runtime(&registry, None)
        .expect("runtime should build");
    // The runtime's language matches the assembled language.
    assert_eq!(rt.lang().tokens.len(), 68);
    assert_eq!(rt.lang().grammar.len(), 56);
}

#[test]
fn assemble_canonical_specific_fragments_present() {
    let kit = handl_kit_data_canonical::assemble().unwrap();
    let lang = kit.lang();

    // --- Token kinds (union/idempotent — re-declared by many emitters) ---
    assert!(lang.tokens.get_by_id("token.identifier").is_some());
    assert!(lang.tokens.get_by_id("token.number").is_some());
    assert!(lang.tokens.get_by_id("token.plus").is_some());
    assert!(lang.tokens.get_by_id("token.arrow").is_some());
    assert!(lang.tokens.get_by_id("token.lparen").is_some());
    assert!(lang.tokens.get_by_id("token.eqeq").is_some());
    assert!(lang.tokens.get_by_id("token.keyword.let").is_some());
    assert!(lang.tokens.get_by_id("token.keyword.match").is_some());
    assert!(lang.tokens.get_by_id("token.bang").is_some());
    assert!(lang.tokens.get_by_id("token.question").is_some());
    // v0.1 new tokens
    assert!(lang.tokens.get_by_id("token.keyword.rec").is_some());
    assert!(lang.tokens.get_by_id("token.keyword.return").is_some());
    assert!(lang.tokens.get_by_id("token.keyword.law").is_some());
    assert!(lang.tokens.get_by_id("token.product").is_some());
    assert!(lang.tokens.get_by_id("token.nullish").is_some());
    assert!(lang.tokens.get_by_id("token.opt_chain").is_some());
    // axiom retained (ADR-0074)
    assert!(lang.tokens.get_by_id("token.keyword.axiom").is_some());
    // vau removed (ADR-0052)
    assert!(lang.tokens.get_by_id("token.vau").is_none());

    // --- Grammar rows ---
    assert!(
        lang.grammar
            .get_by_id("grammar.identifier.prefix")
            .is_some()
    );
    assert!(lang.grammar.get_by_id("grammar.add.infix").is_some());
    assert!(lang.grammar.get_by_id("grammar.eq.infix").is_some());
    assert!(lang.grammar.get_by_id("grammar.call.infix").is_some());
    assert!(lang.grammar.get_by_id("grammar.arrow.infix").is_some());
    assert!(lang.grammar.get_by_id("grammar.array.prefix").is_some());
    assert!(lang.grammar.get_by_id("grammar.object.prefix").is_some());
    assert!(lang.grammar.get_by_id("grammar.match.prefix").is_some());
    assert!(lang.grammar.get_by_id("grammar.paren.prefix").is_some());
    // v0.1 new grammar rules
    assert!(lang.grammar.get_by_id("grammar.product.infix").is_some());
    assert!(lang.grammar.get_by_id("grammar.nullish.infix").is_some());
    assert!(lang.grammar.get_by_id("grammar.rec.prefix").is_some());
    assert!(lang.grammar.get_by_id("grammar.while.prefix").is_some());
    assert!(lang.grammar.get_by_id("grammar.for.prefix").is_some());
    assert!(lang.grammar.get_by_id("grammar.return.prefix").is_some());
    assert!(lang.grammar.get_by_id("grammar.law.prefix").is_some());
    // Removed grammar rules
    assert!(lang.grammar.get_by_id("grammar.vau.prefix").is_none());
    assert!(lang.grammar.get_by_id("grammar.pipe.infix").is_none());
    assert!(lang.grammar.get_by_id("grammar.loop.prefix").is_none());
    // The excluded duplicate grammar rows are absent.
    assert!(lang.grammar.get_by_id("grammar.call.postfix").is_none());
    assert!(lang.grammar.get_by_id("grammar.apply.prefix").is_none());

    // --- Eval schemes ---
    assert!(lang.eval.get_by_id("eval.var").is_some());
    assert!(lang.eval.get_by_id("eval.binary.add").is_some());
    assert!(lang.eval.get_by_id("eval.binary.eq").is_some());
    assert!(lang.eval.get_by_id("eval.call").is_some());
    assert!(lang.eval.get_by_id("eval.function").is_some());
    assert!(lang.eval.get_by_id("eval.if").is_some());
    assert!(lang.eval.get_by_id("eval.array").is_some());
    assert!(lang.eval.get_by_id("eval.object").is_some());
    assert!(lang.eval.get_by_id("eval.match").is_some());
    assert!(lang.eval.get_by_id("eval.resolve").is_some());
    // v0.1 new eval schemes
    assert!(lang.eval.get_by_id("eval.rec").is_some());
    assert!(lang.eval.get_by_id("eval.while").is_some());
    assert!(lang.eval.get_by_id("eval.return").is_some());
    assert!(lang.eval.get_by_id("eval.product").is_some());
    assert!(lang.eval.get_by_id("eval.nullish").is_some());
    // Removed eval schemes
    assert!(lang.eval.get_by_id("eval.loop").is_none());
    assert!(lang.eval.get_by_id("eval.pipe").is_none());
    assert!(lang.eval.get_by_id("eval.handle").is_none());
    // The excluded duplicate eval scheme is absent.
    assert!(lang.eval.get_by_id("eval.call.postfix").is_none());

    // --- AST shapes (union/idempotent) ---
    assert!(lang.ast.get_by_id("ast.var").is_some());
    assert!(lang.ast.get_by_id("ast.binary.add").is_some());
    assert!(lang.ast.get_by_id("ast.function").is_some());
    assert!(lang.ast.get_by_id("ast.resolve").is_some());
    assert!(lang.ast.get_by_id("ast.hello.greet").is_some());
    // v0.1 new AST shapes
    assert!(lang.ast.get_by_id("ast.rec").is_some());
    assert!(lang.ast.get_by_id("ast.product").is_some());
    assert!(lang.ast.get_by_id("ast.nullish").is_some());
    // Removed AST shapes
    assert!(lang.ast.get_by_id("ast.loop").is_none());
    assert!(lang.ast.get_by_id("ast.pipe").is_none());
    assert!(lang.ast.get_by_id("ast.vau").is_none());

    // --- Capabilities ---
    assert!(lang.caps.get_by_id("lex").is_some());
    assert!(lang.caps.get_by_id("parse").is_some());
    assert!(lang.caps.get_by_id("textmate").is_some());
}
