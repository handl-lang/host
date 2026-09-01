//! Integration test: assemble `handl-kit-data-canonical` from the YAML data
//! files and verify the resulting `Language` has the expected fragments
//! (ADR-0044 §6 end state).

use handl_kit_data_canonical;

#[test]
fn assemble_canonical_has_expected_fragment_counts() {
    let kit = handl_kit_data_canonical::assemble().expect("assembly should succeed");
    let lang = kit.lang();

    // Token kinds: 47 from the `tokens` foundation + 5 declared only by
    // grammar/eval emitters (token.let, token.if, token.bang, token.question,
    // token.template) + 3 operator tokens (token.and, token.or,
    // token.percent) + 1 vau keyword token (token.vau) + 3 mood keyword
    // tokens (token.keyword.may, token.keyword.must, token.keyword.when)
    // + 2 new tokens (token.json, token.ellipsis) + 1 proc keyword token
    // (token.keyword.proc) + 1 loop keyword token (token.keyword.loop)
    // + 2 keyword tokens (token.keyword.then, token.keyword.in).
    // token.mood removed (stale placeholder replaced by ADR-0026 keywords).
    // All other re-declarations are union (idempotent) duplicates.
    assert_eq!(lang.tokens.len(), 66);

    // Grammar rows: 56 — one per construct (after excluding the duplicate
    // `grammar.call.postfix` and `grammar.apply.prefix`), plus 3 additional
    // value literal rules (string, bool, null), 1 pow rule, 4 new operator
    // rules (and.infix, or.infix, mod.infix, neg.prefix), 1 vau prefix
    // rule, 1 is infix rule, 4 mood infix rules (may, must, not.infix,
    // when), 2 new rules (json.prefix, ellipsis.prefix), 1 proc prefix
    // rule (grammar.proc.prefix), and 1 loop prefix rule (grammar.loop.prefix).
    // grammar.mood.postfix removed (stale placeholder).
    assert_eq!(lang.grammar.len(), 56);

    // Eval schemes: 55 — one per node kind (after excluding the duplicate
    // `eval.call.postfix` and trimming `eval-core` to its unique `resolve`
    // primitive), plus the `binary.pow` eval scheme,
    // 4 new operator eval schemes (binary.and, binary.or, binary.mod,
    // unary.neg), 1 is eval scheme, 4 mood eval schemes (may, mood.neg,
    // must, when), 1 proc eval scheme (eval.proc), 1 loop eval scheme
    // (eval.loop), 1 rest eval scheme (eval.rest), and 1 bind eval scheme
    // (eval.bind).
    // `eval.vau` is intentionally omitted (ADR-0052: Vau is removed from the
    // kernel — it is a handler concern, not a kernel combinator).
    // eval.mood removed (stale placeholder).
    assert_eq!(lang.eval.len(), 55);

    // AST shapes: 55 — 36 from the `tokens` foundation + 3 unique shapes from
    // other emitters (ast.resolve, ast.hello.greet, ast.json) + the `vau`
    // shape + 4 new operator shapes (binary.and, binary.or, binary.mod,
    // unary.neg) + 1 is shape + 4 mood shapes (may, mood.neg, must, when)
    // + 2 delimited-block shapes (arm, block) from match/handle parsing
    // + 1 proc shape (ast.proc) + 1 loop shape (ast.loop)
    // + 1 rest shape (ast.rest) + 1 bind shape (ast.bind).
    // Duplicates are union (idempotent).
    assert_eq!(lang.ast.len(), 55);

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
    assert_eq!(rt.lang().tokens.len(), 66);
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
    assert!(lang.tokens.get_by_id("token.vau").is_some());

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
    assert!(lang.grammar.get_by_id("grammar.vau.prefix").is_some());
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
    assert!(lang.eval.get_by_id("eval.handle").is_some());
    assert!(lang.eval.get_by_id("eval.resolve").is_some());
    // The excluded duplicate eval scheme is absent.
    assert!(lang.eval.get_by_id("eval.call.postfix").is_none());

    // --- AST shapes (union/idempotent) ---
    assert!(lang.ast.get_by_id("ast.var").is_some());
    assert!(lang.ast.get_by_id("ast.binary.add").is_some());
    assert!(lang.ast.get_by_id("ast.function").is_some());
    assert!(lang.ast.get_by_id("ast.resolve").is_some());
    assert!(lang.ast.get_by_id("ast.hello.greet").is_some());

    // --- Capabilities ---
    assert!(lang.caps.get_by_id("lex").is_some());
    assert!(lang.caps.get_by_id("parse").is_some());
    assert!(lang.caps.get_by_id("textmate").is_some());
}
