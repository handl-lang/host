//! Integration test: assemble `handl-kit-data-canonical` from the YAML data
//! files and verify the resulting `Language` has the expected fragments
//! (ADR-0044 §6 end state).

use handl_kit_data_canonical;

#[test]
fn assemble_canonical_has_expected_fragment_counts() {
    let kit = handl_kit_data_canonical::assemble().expect("assembly should succeed");
    let lang = kit.lang();

    // Token kinds: 47 from the `tokens` foundation + 6 declared only by
    // grammar/eval emitters (token.let, token.if, token.bang, token.question,
    // token.template, token.mood). All other re-declarations are union
    // (idempotent) duplicates.
    assert_eq!(lang.tokens.len(), 53);

    // Grammar rows: 43 — one per construct (after excluding the duplicate
    // `grammar.call.postfix` and `grammar.apply.prefix`), plus 3 additional
    // value literal rules (string, bool, null) and 1 pow rule.
    assert_eq!(lang.grammar.len(), 43);

    // Eval schemes: 43 — one per node kind (after excluding the duplicate
    // `eval.call.postfix` and trimming `eval-core` to its unique `resolve`
    // primitive), plus the `binary.pow` eval scheme.
    assert_eq!(lang.eval.len(), 43);

    // AST shapes: 39 — 36 from the `tokens` foundation + 3 unique shapes from
    // other emitters (ast.resolve, ast.hello.greet, ast.json). Duplicates are
    // union (idempotent).
    assert_eq!(lang.ast.len(), 39);

    // Capabilities: lex + parse (from `tokens`) + textmate (from `textmate`).
    assert_eq!(lang.caps.len(), 3);

    // No options in the canonical kit.
    assert_eq!(lang.options.len(), 0);
}

#[test]
fn assemble_canonical_runtime_config_is_pratt_tree_walk() {
    let kit = handl_kit_data_canonical::assemble().unwrap();
    assert_eq!(kit.runtime_config().parse.as_deref(), Some("pratt"));
    assert_eq!(kit.runtime_config().eval.as_deref(), Some("tree-walk"));
}

#[test]
fn assemble_canonical_into_runtime_binds_engines() {
    let kit = handl_kit_data_canonical::assemble().unwrap();
    let rt = kit.into_runtime(None).expect("runtime should build");
    // The runtime's language matches the assembled language.
    assert_eq!(rt.lang().tokens.len(), 53);
    assert_eq!(rt.lang().grammar.len(), 43);
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

    // --- Grammar rows ---
    assert!(lang.grammar.get_by_id("grammar.identifier.prefix").is_some());
    assert!(lang.grammar.get_by_id("grammar.add.infix").is_some());
    assert!(lang.grammar.get_by_id("grammar.eq.infix").is_some());
    assert!(lang.grammar.get_by_id("grammar.call.infix").is_some());
    assert!(lang.grammar.get_by_id("grammar.arrow.infix").is_some());
    assert!(lang.grammar.get_by_id("grammar.array.prefix").is_some());
    assert!(lang.grammar.get_by_id("grammar.object.prefix").is_some());
    assert!(lang.grammar.get_by_id("grammar.match.prefix").is_some());
    assert!(lang.grammar.get_by_id("grammar.paren.prefix").is_some());
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
