//! Integration test: assemble `handl-kit-data-canonical` from the YAML data
//! files and verify the resulting `Language` has the expected fragments
//! (ADR-0044 §6 end state, v0.1 surface syntax ADR-0074).

use handl_kit_data_canonical;

#[test]
fn assemble_canonical_has_expected_fragment_counts() {
    let kit = handl_kit_data_canonical::assemble().expect("assembly should succeed");
    let lang = kit.lang();

    // Token kinds: 79 — v0.1 surface syntax (ADR-0074) + `::`/`data`
    // additions (ADR-0079/0082) + arrow-combinator surface syntax
    // (`token.keyword.{app,arr,first,left,right,second}` + `token.sum` for
    // `+++`); `token.let` removed (`let` fully removed —
    // ADR-0079 completed) while `token.keyword.let` stays as a tombstone so
    // `let` remains a reserved word (no grammar row → parse error).
    // +7 graded-theory tokens (ADR-0100): `token.fat_arrow` (`=>`) and
    // `token.keyword.{sort,grade,grades,op,handler,rewrite}`.
    // +1 `token.keyword.handle` for the `handle` expression keyword.
    // +1 `token.tilde` for the graded arrow `~EXPR~>` syntax.
    assert_eq!(lang.tokens.len(), 105);

    // Lexer rules: 91 v0.1 + 8 Option B frontier operators/keywords.
    assert_eq!(lang.lexer_rules.len(), 103);

    // Grammar rows: 67 — v0.1 surface syntax (ADR-0074) + `::`/`data`
    // additions; `|` pipe re-added for `data` constructor alternatives;
    // `grammar.let.prefix` removed (`let` fully removed);
    // `grammar.axiom.prefix` added (`axiom` aliases to a `law` node);
    // arrow combinators added (`arr`/`first`/`second`/`left`/`right`
    // prefixes + `sum`/`app` infixes).
    // +5 graded-theory rows (ADR-0100): `grammar.{sort,grade,op,handler,
    // rewrite}.prefix`; `grammar.law.prefix`/`grammar.axiom.prefix` were
    // re-shaped in place as 2-cells `name : source => target`.
    // +2 `grammar.{with,handle}.prefix` for handler-install expressions.
    // +1 `grammar.dim_arrow.infix` for `~[N]~>`.
    // +9 Option B infix/id2 rows - 8 removed call-style rows = +1.
    assert_eq!(lang.grammar.len(), 90);

    // +13 Term frontier eval rules + 1 transport rule.
    // +1 `eval.dimension` for the dimension specialization `f<...>`
    // (ADR-0106 §11/§12 — evaluates as application of the target to the
    // dimension arguments, mirroring `eval.call`).
    assert_eq!(lang.eval.len(), 80);

    // AST shapes: 68 — v0.1 surface syntax (ADR-0074) + `data`/`entry`/
    // `flow` (arrow-parser refinement roles) additions + arrow-combinator
    // shapes (`arr`/`first`/`second`/`left`/`right`/`sum`/`app`);
    // `ast.let` removed.
    // +5 graded-theory shapes (ADR-0100): `ast.{sort,grade,op,handler,
    // rewrite}`; `ast.law` was re-shaped in place (children
    // [name, source, target]).
    // +2 `ast.{with,handle}` for the surface syntax.
    // +1 `ast.dim_arrow` for `~[N]~>`.
    // +13 Term frontier shapes + 1 transport shape.
    // +1 `ast.dimension` for the dimension specialization `f<...>`
    // (ADR-0106 §11 — the bracketed spelling adjacent to a name-like
    // target, distinguishable from the `<` comparison operator).
    assert_eq!(lang.ast.len(), 91);

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
    assert_eq!(kit.runtime_config().typecheck, "nbe");
}

#[test]
fn assemble_canonical_into_runtime_binds_engines() {
    use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
    use handl_plugin_engine_lex_pattern::DataLexerEngine;
    use handl_plugin_engine_parse_pratt::PrattEngine;
    use handl_runtime::{EngineRegistry, Runtime};
    use std::rc::Rc;
    let kit = handl_kit_data_canonical::assemble().unwrap();
    let mut reg = EngineRegistry::new();
    reg.register_lex("data", Box::new(DataLexerEngine::new()));
    reg.register_parse("pratt", PrattEngine::new());
    reg.register_eval("tree-walk", Box::new(TreeWalkEngine::new()));
    let registry = Rc::new(reg);
    // Bind only the lex/parse/eval engines this test verifies; the typecheck
    // engine is selected by `runtime.typecheck` and tested elsewhere.
    let mut config = kit.runtime_config().clone();
    config.typecheck.clear();
    let rt = Runtime::from_registry(kit.lang().clone(), &registry, &config)
        .expect("runtime should build");
    // The runtime's language matches the assembled language.
    assert_eq!(rt.lang().tokens.len(), 105);
    assert_eq!(rt.lang().grammar.len(), 90);
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
    // `let` is removed: the lexed binding token is gone; the keyword token
    // remains as a tombstone (reserved word, no grammar row).
    assert!(lang.tokens.get_by_id("token.let").is_none());
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
    // v0.1 do-while (post-test loop, ADR-0074 A.6)
    assert!(lang.grammar.get_by_id("grammar.do-while.infix").is_some());
    // Removed grammar rules
    assert!(lang.grammar.get_by_id("grammar.vau.prefix").is_none());
    // `|` pipe re-added for `data` constructor alternatives (ADR-0082).
    assert!(lang.grammar.get_by_id("grammar.pipe.infix").is_some());
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
    // `eval.handle` is part of the surface syntax now.
    assert!(lang.eval.get_by_id("eval.handle").is_some());
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
