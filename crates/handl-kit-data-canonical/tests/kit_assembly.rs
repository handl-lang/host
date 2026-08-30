//! Integration test: assemble `handl-kit-data-canonical` from the full set
//! of Phase 4 emitters and verify the resulting `Language` has the expected
//! fragment counts (ADR-0044 Phase 5 full-language coverage).
//!
//! The canonical kit merges the foundation `tokens` emitter plus every
//! grammar/eval emitter (37 fragment sources — all 39 Phase 4 emitters except
//! the pure alternative emitters `arrow` and `function-literal`), with
//! `with_exclude` dropping the non-canonical duplicate behavioral fragments
//! (`eval.call.postfix`, `grammar.call.postfix`, `grammar.apply.prefix`).

use handl_kit_data::{DataKit, DataKitManifest, FragmentSourceEntry};
use handl_kit_data_canonical;
use handl_runtime::RuntimeConfig;

/// Build the canonical manifest directly (rather than via the canonical
/// crate's `manifest()`) to exercise the assembler end-to-end from fragment
/// sources.
///
/// This mirrors [`handl_kit_data_canonical::manifest()`]: the foundation
/// `tokens` emitter first, then the rest, with the duplicate behavioral
/// fragments excluded.
fn canonical_manifest() -> DataKitManifest {
    DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter("tokens", "tokens"))
        .with_fragment(FragmentSourceEntry::emitter("identifier", "identifier"))
        .with_fragment(FragmentSourceEntry::emitter("value", "value"))
        .with_fragment(FragmentSourceEntry::emitter("arith", "arith"))
        .with_fragment(FragmentSourceEntry::emitter("let", "let"))
        .with_fragment(FragmentSourceEntry::emitter("call", "call"))
        .with_fragment(FragmentSourceEntry::emitter("branch", "branch"))
        .with_fragment(FragmentSourceEntry::emitter("function", "function"))
        .with_fragment(FragmentSourceEntry::emitter("cmp", "cmp"))
        .with_fragment(FragmentSourceEntry::emitter("assign", "assign"))
        .with_fragment(FragmentSourceEntry::emitter("pipe", "pipe"))
        .with_fragment(FragmentSourceEntry::emitter("flow", "flow"))
        .with_fragment(FragmentSourceEntry::emitter("fanout", "fanout"))
        .with_fragment(FragmentSourceEntry::emitter("ternary", "ternary"))
        .with_fragment(FragmentSourceEntry::emitter("not", "not"))
        .with_fragment(FragmentSourceEntry::emitter("paren", "paren"))
        .with_fragment(FragmentSourceEntry::emitter("typeof", "typeof"))
        .with_fragment(FragmentSourceEntry::emitter("array", "array"))
        .with_fragment(FragmentSourceEntry::emitter("object", "object"))
        .with_fragment(FragmentSourceEntry::emitter("match", "match"))
        .with_fragment(FragmentSourceEntry::emitter("handle", "handle"))
        .with_fragment(FragmentSourceEntry::emitter("postfix-call", "postfix-call"))
        .with_fragment(FragmentSourceEntry::emitter(
            "postfix-field",
            "postfix-field",
        ))
        .with_fragment(FragmentSourceEntry::emitter(
            "postfix-index",
            "postfix-index",
        ))
        .with_fragment(FragmentSourceEntry::emitter(
            "postfix-mood",
            "postfix-mood",
        ))
        .with_fragment(FragmentSourceEntry::emitter("apply", "apply"))
        .with_fragment(FragmentSourceEntry::emitter("template", "template"))
        .with_fragment(FragmentSourceEntry::emitter("type-annot", "type-annot"))
        .with_fragment(FragmentSourceEntry::emitter("textmate", "textmate"))
        .with_fragment(FragmentSourceEntry::emitter("stmt-let", "stmt-let"))
        .with_fragment(FragmentSourceEntry::emitter("stmt-const", "stmt-const"))
        .with_fragment(FragmentSourceEntry::emitter("stmt-import", "stmt-import"))
        .with_fragment(FragmentSourceEntry::emitter("stmt-export", "stmt-export"))
        .with_fragment(FragmentSourceEntry::emitter("stmt-module", "stmt-module"))
        .with_fragment(FragmentSourceEntry::emitter("eval-core", "eval-core"))
        .with_fragment(FragmentSourceEntry::emitter("eval-hello", "eval-hello"))
        .with_fragment(FragmentSourceEntry::emitter("eval-json", "eval-json"))
        .with_exclude(
            handl_kit_data_canonical::EXCLUDED_FRAGMENT_IDS
                .iter()
                .map(|s| s.to_string())
                .collect(),
        )
        .with_runtime(
            RuntimeConfig::new()
                .with_parse("pratt")
                .with_eval("tree-walk"),
        )
}

#[test]
fn assemble_canonical_has_expected_fragment_counts() {
    let reg = handl_kit_data_canonical::registry();
    let manifest = canonical_manifest();
    let kit = DataKit::assemble(&manifest, &reg).expect("assembly should succeed");
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
fn assemble_canonical_via_crate_helper_matches() {
    // The canonical crate's own `assemble()` should produce the same counts.
    let kit = handl_kit_data_canonical::assemble().expect("assembly should succeed");
    let lang = kit.lang();
    assert_eq!(lang.tokens.len(), 53);
    assert_eq!(lang.grammar.len(), 43);
    assert_eq!(lang.eval.len(), 43);
    assert_eq!(lang.ast.len(), 39);
    assert_eq!(lang.caps.len(), 3);
}

#[test]
fn assemble_canonical_runtime_config_is_pratt_tree_walk() {
    let reg = handl_kit_data_canonical::registry();
    let kit = DataKit::assemble(&canonical_manifest(), &reg).unwrap();
    assert_eq!(kit.runtime_config().parse.as_deref(), Some("pratt"));
    assert_eq!(kit.runtime_config().eval.as_deref(), Some("tree-walk"));
}

#[test]
fn assemble_canonical_into_runtime_binds_engines() {
    let reg = handl_kit_data_canonical::registry();
    let kit = DataKit::assemble(&canonical_manifest(), &reg).unwrap();
    let rt = kit.into_runtime(None).expect("runtime should build");
    // The runtime's language matches the assembled language.
    assert_eq!(rt.lang().tokens.len(), 53);
    assert_eq!(rt.lang().grammar.len(), 43);
}

#[test]
fn assemble_canonical_specific_fragments_present() {
    let reg = handl_kit_data_canonical::registry();
    let kit = DataKit::assemble(&canonical_manifest(), &reg).unwrap();
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

#[test]
fn assemble_canonical_exclude_drops_a_fragment() {
    let reg = handl_kit_data_canonical::registry();
    // Exclude an additional token kind on top of the canonical excludes.
    // (`with_exclude` replaces the list, so re-include the canonical excludes.)
    let mut excludes: Vec<String> = handl_kit_data_canonical::EXCLUDED_FRAGMENT_IDS
        .iter()
        .map(|s| s.to_string())
        .collect();
    excludes.push("token.slash".into());
    let manifest = canonical_manifest().with_exclude(excludes);
    let kit = DataKit::assemble(&manifest, &reg).unwrap();
    // Excluding token.slash drops one token kind (53 -> 52). Its grammar row
    // (grammar.div.infix) is NOT excluded (different id), so grammar stays at
    // 43.
    assert_eq!(kit.lang().tokens.len(), 52);
    assert!(kit.lang().tokens.get_by_id("token.slash").is_none());
    assert_eq!(kit.lang().grammar.len(), 43);
}
