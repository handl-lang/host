//! Inline unit tests for `handl-kit-data` (ADR-0044 Phase 5 coverage).
//!
//! These cover: manifest parsing (serde round-trip), kit assembly (merge in
//! declared order), include/exclude filtering, the exclude-replaces
//! interaction (explicit error, ADR §5), and the `extends` chain.

use super::*;
use handl_emitters::Emitter;
use handl_language::{
    AstShapeRef, CapabilityDecl, EvalSchemeDef, GrammarPosition, GrammarRow, LangError, Language,
    Scheme, TokenKindDef,
};
use handl_runtime::RuntimeConfig;
use serde_json::Value;

// ---------------------------------------------------------------------------
// Test emitters
// ---------------------------------------------------------------------------

/// An emitter that produces a single token kind `id`.
struct TokenEmitter {
    id: String,
}

impl Emitter for TokenEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        let mut lang = Language::new();
        lang.tokens.merge(TokenKindDef::new(&self.id))?;
        Ok(lang)
    }
}

/// An emitter that produces a single token kind `A`.
struct TokenAEmitter;

impl Emitter for TokenAEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        let mut lang = Language::new();
        lang.tokens.merge(TokenKindDef::new("token.A"))?;
        Ok(lang)
    }
}

/// An emitter that produces `A` then `A2` (which `replaces` `A`). Because
/// `FragmentSet::merge` applies replaces destructively within a single
/// `Language`, the emitted `Language` contains only `A2` (with
/// `replaces: ["token.A"]`). When `A2` is later merged into an accumulator
/// that already has `A` (from [`TokenAEmitter`]), the cross-source replace
/// succeeds. This models the kit-level override: a later fragment source
/// overrides an earlier one.
struct TokenAprimeEmitter;

impl Emitter for TokenAprimeEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        let mut lang = Language::new();
        lang.tokens.merge(TokenKindDef::new("token.A"))?;
        lang.tokens
            .merge(TokenKindDef::new("token.A2").with_replaces(vec!["token.A".into()]))?;
        Ok(lang)
    }
}

/// An emitter producing a token + grammar + eval (a minimal construct).
struct ConstructEmitter {
    token_id: &'static str,
    grammar_id: &'static str,
    eval_id: &'static str,
    node_kind: &'static str,
}

impl Emitter for ConstructEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        let mut lang = Language::new();
        lang.tokens.merge(TokenKindDef::new(self.token_id))?;
        lang.grammar.merge(GrammarRow::new(
            self.grammar_id,
            self.token_id,
            GrammarPosition::Prefix,
            AstShapeRef::new(self.node_kind, 0),
        ))?;
        lang.eval.merge(EvalSchemeDef::new(
            self.eval_id,
            self.node_kind,
            Scheme::arr_native_lit("const", vec![], Value::Null),
        ))?;
        Ok(lang)
    }
}

/// An emitter declaring a capability.
struct CapEmitter {
    cap_id: &'static str,
}

impl Emitter for CapEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        let mut lang = Language::new();
        lang.caps.merge(CapabilityDecl::new(self.cap_id))?;
        Ok(lang)
    }
}

/// An emitter that produces an AST shape and an option schema (to exercise the
/// `ast` and `options` merge paths in `merge_filtered`).
struct AstOptionEmitter;

impl Emitter for AstOptionEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        let mut lang = Language::new();
        lang.ast
            .merge(handl_language::AstShapeDef::new("shape.x", "x"))?;
        lang.options
            .merge(handl_language::OptionSchema::new("opt.x"))?;
        Ok(lang)
    }
}

/// Build a registry with seven inline test emitters (used by assembly tests).
fn seven_emitter_registry() -> EmitterRegistry {
    let mut reg = EmitterRegistry::new();
    reg.register_emitter("identifier", || {
        Box::new(ConstructEmitter {
            token_id: "token.identifier",
            grammar_id: "grammar.identifier.prefix",
            eval_id: "eval.var",
            node_kind: "var",
        })
    });
    reg.register_emitter("value", || {
        Box::new(ConstructEmitter {
            token_id: "token.number",
            grammar_id: "grammar.value.number.prefix",
            eval_id: "eval.value.number",
            node_kind: "value.number",
        })
    });
    reg.register_emitter("arith", || {
        Box::new(ConstructEmitter {
            token_id: "token.plus",
            grammar_id: "grammar.add.infix",
            eval_id: "eval.binary.add",
            node_kind: "binary.add",
        })
    });
    reg.register_emitter("let", || {
        Box::new(ConstructEmitter {
            token_id: "token.keyword.let",
            grammar_id: "grammar.let.prefix",
            eval_id: "eval.let",
            node_kind: "let",
        })
    });
    reg.register_emitter("call", || {
        Box::new(ConstructEmitter {
            token_id: "token.lparen",
            grammar_id: "grammar.call.infix",
            eval_id: "eval.call",
            node_kind: "call",
        })
    });
    reg.register_emitter("branch", || {
        Box::new(ConstructEmitter {
            token_id: "token.keyword.if",
            grammar_id: "grammar.if.prefix",
            eval_id: "eval.if",
            node_kind: "if",
        })
    });
    reg.register_emitter("function", || {
        Box::new(ConstructEmitter {
            token_id: "token.arrow",
            grammar_id: "grammar.arrow.infix",
            eval_id: "eval.function",
            node_kind: "function",
        })
    });
    reg
}

// ---------------------------------------------------------------------------
// Manifest parsing / serde round-trip
// ---------------------------------------------------------------------------

#[test]
fn manifest_serde_roundtrip_emitter_sources() {
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter("arith", "arith"))
        .with_fragment(FragmentSourceEntry::emitter("value", "value"))
        .with_include(vec!["eval.binary.add".into()])
        .with_exclude(vec!["token.slash".into()])
        .with_extends("kit-data-canonical")
        .with_runtime(
            RuntimeConfig::new()
                .with_parse("pratt")
                .with_eval("tree-walk"),
        );

    let j = serde_json::to_string(&manifest).unwrap();
    let back: DataKitManifest = serde_json::from_str(&j).unwrap();
    assert_eq!(manifest, back);
}

#[test]
fn manifest_serde_roundtrip_data_file_source() {
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("data", "lang/data.toml"));
    let j = serde_json::to_string(&manifest).unwrap();
    let back: DataKitManifest = serde_json::from_str(&j).unwrap();
    assert_eq!(manifest, back);
    assert!(matches!(
        back.fragments[0].source,
        FragmentSourceSerde::DataFile(_)
    ));
}

#[test]
fn manifest_defaults_are_empty() {
    let m = DataKitManifest::default();
    assert!(m.fragments.is_empty());
    assert!(m.include.is_empty());
    assert!(m.exclude.is_empty());
    assert!(m.extends.is_none());
    assert_eq!(m.runtime, RuntimeConfig::new());
}

#[test]
fn fragment_source_entry_emitter_constructor() {
    let e = FragmentSourceEntry::emitter("name", "arith");
    assert_eq!(e.name, "name");
    assert_eq!(e.source, FragmentSourceSerde::Emitter("arith".into()));
}

#[test]
fn fragment_source_entry_data_file_constructor() {
    let e = FragmentSourceEntry::data_file("name", "path.toml");
    assert_eq!(e.name, "name");
    assert_eq!(e.source, FragmentSourceSerde::DataFile("path.toml".into()));
}

// ---------------------------------------------------------------------------
// EmitterRegistry
// ---------------------------------------------------------------------------

#[test]
fn registry_resolves_registered_emitter() {
    let mut reg = EmitterRegistry::new();
    reg.register_emitter("tok", || {
        Box::new(TokenEmitter {
            id: "token.x".into(),
        })
    });
    let source = reg
        .resolve_source(&FragmentSourceSerde::Emitter("tok".into()))
        .unwrap();
    let lang = produce_language(&source).unwrap();
    assert_eq!(lang.tokens.len(), 1);
}

#[test]
fn registry_unknown_emitter_errors() {
    let reg = EmitterRegistry::new();
    let err = reg
        .resolve_source(&FragmentSourceSerde::Emitter("ghost".into()))
        .unwrap_err();
    assert!(matches!(err, LangError::Engine { .. }));
    assert_eq!(err, LangError::engine("unknown emitter: ghost"));
}

#[test]
fn registry_resolves_data_file_source() {
    let reg = EmitterRegistry::new();
    let source = reg
        .resolve_source(&FragmentSourceSerde::DataFile("x.toml".into()))
        .unwrap();
    assert!(matches!(source, FragmentSource::DataFile(_)));
}

#[test]
fn registry_resolves_kit_manifest() {
    let mut reg = EmitterRegistry::new();
    let manifest = DataKitManifest::new().with_extends("none");
    reg.register_kit("my-kit", manifest.clone());
    let resolved = reg.resolve_kit("my-kit").unwrap();
    assert_eq!(resolved.extends, Some("none".into()));
}

#[test]
fn registry_unknown_kit_returns_none() {
    let reg = EmitterRegistry::new();
    assert!(reg.resolve_kit("ghost").is_none());
}

// ---------------------------------------------------------------------------
// Kit assembly: merge fragments in declared order
// ---------------------------------------------------------------------------

#[test]
fn assemble_merges_fragments_in_declared_order() {
    let mut reg = EmitterRegistry::new();
    reg.register_emitter("a", || {
        Box::new(TokenEmitter {
            id: "token.a".into(),
        })
    });
    reg.register_emitter("b", || {
        Box::new(TokenEmitter {
            id: "token.b".into(),
        })
    });
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter("a", "a"))
        .with_fragment(FragmentSourceEntry::emitter("b", "b"));
    let kit = DataKit::assemble(&manifest, &reg).unwrap();
    assert_eq!(kit.lang().tokens.len(), 2);
    assert!(kit.lang().tokens.get_by_id("token.a").is_some());
    assert!(kit.lang().tokens.get_by_id("token.b").is_some());
}

#[test]
fn assemble_produces_language_and_runtime_config() {
    let mut reg = EmitterRegistry::new();
    reg.register_emitter("tok", || {
        Box::new(TokenEmitter {
            id: "token.x".into(),
        })
    });
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter("tok", "tok"))
        .with_runtime(
            RuntimeConfig::new()
                .with_parse("pratt")
                .with_eval("tree-walk"),
        );
    let kit = DataKit::assemble(&manifest, &reg).unwrap();
    assert_eq!(kit.lang().tokens.len(), 1);
    assert_eq!(kit.runtime_config().parse, "pratt");
    assert_eq!(kit.runtime_config().eval, "tree-walk");
}

#[test]
fn assemble_construct_emitter_merges_all_three_sets() {
    let mut reg = EmitterRegistry::new();
    reg.register_emitter("c", || {
        Box::new(ConstructEmitter {
            token_id: "token.c",
            grammar_id: "g.c",
            eval_id: "e.c",
            node_kind: "c",
        })
    });
    let manifest = DataKitManifest::new().with_fragment(FragmentSourceEntry::emitter("c", "c"));
    let kit = DataKit::assemble(&manifest, &reg).unwrap();
    assert_eq!(kit.lang().tokens.len(), 1);
    assert_eq!(kit.lang().grammar.len(), 1);
    assert_eq!(kit.lang().eval.len(), 1);
}

// ---------------------------------------------------------------------------
// Include / exclude
// ---------------------------------------------------------------------------

#[test]
fn exclude_skips_fragment_by_id() {
    let mut reg = EmitterRegistry::new();
    reg.register_emitter("a", || {
        Box::new(TokenEmitter {
            id: "token.a".into(),
        })
    });
    reg.register_emitter("b", || {
        Box::new(TokenEmitter {
            id: "token.b".into(),
        })
    });
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter("a", "a"))
        .with_fragment(FragmentSourceEntry::emitter("b", "b"))
        .with_exclude(vec!["token.a".into()]);
    let kit = DataKit::assemble(&manifest, &reg).unwrap();
    assert_eq!(kit.lang().tokens.len(), 1);
    assert!(kit.lang().tokens.get_by_id("token.a").is_none());
    assert!(kit.lang().tokens.get_by_id("token.b").is_some());
}

#[test]
fn include_keeps_only_listed_ids() {
    let mut reg = EmitterRegistry::new();
    reg.register_emitter("a", || {
        Box::new(TokenEmitter {
            id: "token.a".into(),
        })
    });
    reg.register_emitter("b", || {
        Box::new(TokenEmitter {
            id: "token.b".into(),
        })
    });
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter("a", "a"))
        .with_fragment(FragmentSourceEntry::emitter("b", "b"))
        .with_include(vec!["token.a".into()]);
    let kit = DataKit::assemble(&manifest, &reg).unwrap();
    assert_eq!(kit.lang().tokens.len(), 1);
    assert!(kit.lang().tokens.get_by_id("token.a").is_some());
    assert!(kit.lang().tokens.get_by_id("token.b").is_none());
}

#[test]
fn include_empty_keeps_all() {
    let mut reg = EmitterRegistry::new();
    reg.register_emitter("a", || {
        Box::new(TokenEmitter {
            id: "token.a".into(),
        })
    });
    let manifest = DataKitManifest::new().with_fragment(FragmentSourceEntry::emitter("a", "a"));
    let kit = DataKit::assemble(&manifest, &reg).unwrap();
    assert_eq!(kit.lang().tokens.len(), 1);
}

#[test]
fn exclude_takes_precedence_over_include() {
    // A fragment in both include and exclude is excluded.
    let mut reg = EmitterRegistry::new();
    reg.register_emitter("a", || {
        Box::new(TokenEmitter {
            id: "token.a".into(),
        })
    });
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter("a", "a"))
        .with_include(vec!["token.a".into()])
        .with_exclude(vec!["token.a".into()]);
    let kit = DataKit::assemble(&manifest, &reg).unwrap();
    assert_eq!(kit.lang().tokens.len(), 0);
}

#[test]
fn exclude_across_multiple_sets() {
    // Excluding a grammar id skips only that grammar row, not the token.
    let mut reg = EmitterRegistry::new();
    reg.register_emitter("c", || {
        Box::new(ConstructEmitter {
            token_id: "token.c",
            grammar_id: "g.c",
            eval_id: "e.c",
            node_kind: "c",
        })
    });
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter("c", "c"))
        .with_exclude(vec!["g.c".into()]);
    let kit = DataKit::assemble(&manifest, &reg).unwrap();
    assert_eq!(kit.lang().tokens.len(), 1);
    assert_eq!(kit.lang().grammar.len(), 0);
    assert_eq!(kit.lang().eval.len(), 1);
}

// ---------------------------------------------------------------------------
// Exclude a fragment that others `replaces` → explicit error (ADR §5)
// ---------------------------------------------------------------------------

#[test]
fn exclude_replaces_target_errors_explicitly() {
    // Scenario (ADR §5): source1 emits `token.A`; source2 emits `token.A2`
    // which `replaces: ["token.A"]` (source2 emits A then A2, so its emitted
    // Language contains only A2 with the replaces declaration). Excluding
    // `token.A` means A is never merged into the accumulator, so when A2 is
    // merged its replace target is absent → `UnknownReplace` error. This is
    // the explicit, non-silent behavior the ADR requires.
    let mut reg = EmitterRegistry::new();
    reg.register_emitter("a", || Box::new(TokenAEmitter));
    reg.register_emitter("aprime", || Box::new(TokenAprimeEmitter));
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter("a", "a"))
        .with_fragment(FragmentSourceEntry::emitter("aprime", "aprime"))
        .with_exclude(vec!["token.A".into()]);
    let err = DataKit::assemble(&manifest, &reg).unwrap_err();
    assert_eq!(
        err,
        LangError::UnknownReplace {
            missing: "token.A".into()
        }
    );
}

#[test]
fn replace_without_exclude_succeeds() {
    // Control: without excluding A, source1's A merges into the accumulator,
    // then source2's A2 (replaces A) merges and replaces A cleanly.
    let mut reg = EmitterRegistry::new();
    reg.register_emitter("a", || Box::new(TokenAEmitter));
    reg.register_emitter("aprime", || Box::new(TokenAprimeEmitter));
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter("a", "a"))
        .with_fragment(FragmentSourceEntry::emitter("aprime", "aprime"));
    let kit = DataKit::assemble(&manifest, &reg).unwrap();
    assert_eq!(kit.lang().tokens.len(), 1);
    assert!(kit.lang().tokens.get_by_id("token.A").is_none());
    assert!(kit.lang().tokens.get_by_id("token.A2").is_some());
}

// ---------------------------------------------------------------------------
// `extends` chain
// ---------------------------------------------------------------------------

#[test]
fn extends_assembles_parent_then_child() {
    let mut reg = EmitterRegistry::new();
    reg.register_emitter("parent-tok", || {
        Box::new(TokenEmitter {
            id: "token.parent".into(),
        })
    });
    reg.register_emitter("child-tok", || {
        Box::new(TokenEmitter {
            id: "token.child".into(),
        })
    });
    let parent = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter("parent-tok", "parent-tok"));
    reg.register_kit("parent-kit", parent);
    let child = DataKitManifest::new()
        .with_extends("parent-kit")
        .with_fragment(FragmentSourceEntry::emitter("child-tok", "child-tok"));
    let kit = DataKit::assemble(&child, &reg).unwrap();
    assert_eq!(kit.lang().tokens.len(), 2);
    assert!(kit.lang().tokens.get_by_id("token.parent").is_some());
    assert!(kit.lang().tokens.get_by_id("token.child").is_some());
}

#[test]
fn extends_unknown_parent_errors() {
    let reg = EmitterRegistry::new();
    let manifest = DataKitManifest::new().with_extends("ghost-kit");
    let err = DataKit::assemble(&manifest, &reg).unwrap_err();
    assert_eq!(err, LangError::engine("unknown parent kit: ghost-kit"));
}

#[test]
fn extends_child_overrides_parent_fragment() {
    // Parent declares token.X. Child's emitter emits X then Y (replaces X),
    // yielding a Language containing only Y (with replaces:["token.X"]).
    // When Y merges into the accumulator (which has X from the parent), Y
    // replaces X — the cross-kit override.
    let mut reg = EmitterRegistry::new();
    reg.register_emitter("x", || {
        Box::new(TokenEmitter {
            id: "token.X".into(),
        })
    });
    struct ReplaceXEmitter;
    impl Emitter for ReplaceXEmitter {
        fn emit(&self) -> Result<Language, LangError> {
            let mut lang = Language::new();
            lang.tokens.merge(TokenKindDef::new("token.X"))?;
            lang.tokens
                .merge(TokenKindDef::new("token.Y").with_replaces(vec!["token.X".into()]))?;
            Ok(lang)
        }
    }
    reg.register_emitter("replace-x", || Box::new(ReplaceXEmitter));
    let parent = DataKitManifest::new().with_fragment(FragmentSourceEntry::emitter("x", "x"));
    reg.register_kit("parent-kit", parent);
    let child = DataKitManifest::new()
        .with_extends("parent-kit")
        .with_fragment(FragmentSourceEntry::emitter("replace-x", "replace-x"));
    let kit = DataKit::assemble(&child, &reg).unwrap();
    assert!(kit.lang().tokens.get_by_id("token.X").is_none());
    assert!(kit.lang().tokens.get_by_id("token.Y").is_some());
    assert_eq!(kit.lang().tokens.len(), 1);
}

#[test]
fn extends_chain_union_of_capabilities() {
    // Parent declares cap.a; child declares cap.b → union.
    let mut reg = EmitterRegistry::new();
    reg.register_emitter("cap-a", || Box::new(CapEmitter { cap_id: "cap.a" }));
    reg.register_emitter("cap-b", || Box::new(CapEmitter { cap_id: "cap.b" }));
    let parent =
        DataKitManifest::new().with_fragment(FragmentSourceEntry::emitter("cap-a", "cap-a"));
    reg.register_kit("parent-kit", parent);
    let child = DataKitManifest::new()
        .with_extends("parent-kit")
        .with_fragment(FragmentSourceEntry::emitter("cap-b", "cap-b"));
    let kit = DataKit::assemble(&child, &reg).unwrap();
    assert_eq!(kit.lang().caps.len(), 2);
    assert!(kit.lang().caps.get_by_id("cap.a").is_some());
    assert!(kit.lang().caps.get_by_id("cap.b").is_some());
}

// ---------------------------------------------------------------------------
// Data-file source (not yet implemented)
// ---------------------------------------------------------------------------

#[test]
fn data_file_source_returns_not_implemented_style_error() {
    let reg = EmitterRegistry::new();
    let manifest =
        DataKitManifest::new().with_fragment(FragmentSourceEntry::data_file("d", "x.toml"));
    let err = DataKit::assemble(&manifest, &reg).unwrap_err();
    assert!(matches!(err, LangError::Engine { .. }));
}

// ---------------------------------------------------------------------------
// into_runtime: engine selection from data
// ---------------------------------------------------------------------------

#[test]
fn into_runtime_binds_pratt_and_tree_walk() {
    use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
    use handl_plugin_engine_lex_pattern::DataLexerEngine;
    use handl_plugin_engine_parse_pratt::PrattEngine;
    use handl_runtime::EngineRegistry;
    use std::rc::Rc;

    let mut reg = EmitterRegistry::new();
    reg.register_emitter("tok", || {
        Box::new(TokenEmitter {
            id: "token.x".into(),
        })
    });
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter("tok", "tok"))
        .with_runtime(
            RuntimeConfig::new()
                .with_parse("pratt")
                .with_eval("tree-walk"),
        );
    let kit = DataKit::assemble(&manifest, &reg).unwrap();
    let mut engine_reg = EngineRegistry::new();
    engine_reg.register_lex("data", Box::new(DataLexerEngine::new()));
    engine_reg.register_parse("pratt", PrattEngine::new());
    engine_reg.register_eval("tree-walk", Box::new(TreeWalkEngine::new()));
    let engine_reg = Rc::new(engine_reg);
    let rt = kit.into_runtime(&engine_reg, None).unwrap();
    assert_eq!(rt.lang().tokens.len(), 1);
}

#[test]
fn into_runtime_unknown_eval_engine_errors() {
    use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
    use handl_plugin_engine_lex_pattern::DataLexerEngine;
    use handl_plugin_engine_parse_pratt::PrattEngine;
    use handl_runtime::EngineRegistry;
    use std::rc::Rc;

    let mut reg = EmitterRegistry::new();
    reg.register_emitter("tok", || {
        Box::new(TokenEmitter {
            id: "token.x".into(),
        })
    });
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter("tok", "tok"))
        .with_runtime(RuntimeConfig::new().with_eval("nonsense"));
    let kit = DataKit::assemble(&manifest, &reg).unwrap();
    let mut engine_reg = EngineRegistry::new();
    engine_reg.register_lex("data", Box::new(DataLexerEngine::new()));
    engine_reg.register_parse("pratt", PrattEngine::new());
    engine_reg.register_eval("tree-walk", Box::new(TreeWalkEngine::new()));
    let engine_reg = Rc::new(engine_reg);
    let err = kit.into_runtime(&engine_reg, None).unwrap_err();
    assert_eq!(err, LangError::engine("unknown eval engine: nonsense"));
}

// ---------------------------------------------------------------------------
// Full seven-emitter assembly (representative subset)
// ---------------------------------------------------------------------------

#[test]
fn assemble_seven_emitters_produces_expected_counts() {
    let reg = seven_emitter_registry();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter("identifier", "identifier"))
        .with_fragment(FragmentSourceEntry::emitter("value", "value"))
        .with_fragment(FragmentSourceEntry::emitter("arith", "arith"))
        .with_fragment(FragmentSourceEntry::emitter("let", "let"))
        .with_fragment(FragmentSourceEntry::emitter("call", "call"))
        .with_fragment(FragmentSourceEntry::emitter("branch", "branch"))
        .with_fragment(FragmentSourceEntry::emitter("function", "function"))
        .with_runtime(
            RuntimeConfig::new()
                .with_parse("pratt")
                .with_eval("tree-walk"),
        );
    let kit = DataKit::assemble(&manifest, &reg).unwrap();
    // 7 emitters contribute tokens, grammar rows, and eval schemes. The
    // exact counts are asserted in the integration test; here we sanity-check
    // non-emptiness.
    assert!(kit.lang().tokens.len() > 0);
    assert!(kit.lang().grammar.len() > 0);
    assert!(kit.lang().eval.len() > 0);
}

// ---------------------------------------------------------------------------
// Coverage: Debug impls, Default, and ast/options merge paths
// ---------------------------------------------------------------------------

#[test]
fn fragment_source_debug_emitter() {
    let mut reg = EmitterRegistry::new();
    reg.register_emitter("tok", || {
        Box::new(TokenEmitter {
            id: "token.x".into(),
        })
    });
    let source = reg
        .resolve_source(&FragmentSourceSerde::Emitter("tok".into()))
        .unwrap();
    let s = format!("{:?}", source);
    assert!(s.contains("Emitter"));
}

#[test]
fn fragment_source_debug_data_file() {
    let reg = EmitterRegistry::new();
    let source = reg
        .resolve_source(&FragmentSourceSerde::DataFile("x.toml".into()))
        .unwrap();
    let s = format!("{:?}", source);
    assert!(s.contains("DataFile"));
    assert!(s.contains("x.toml"));
}

#[test]
fn emitter_registry_default_is_empty() {
    let reg = EmitterRegistry::default();
    assert!(reg.resolve_emitter("anything").is_none());
    assert!(reg.resolve_kit("anything").is_none());
}

#[test]
fn data_kit_debug_formats() {
    let mut reg = EmitterRegistry::new();
    reg.register_emitter("tok", || {
        Box::new(TokenEmitter {
            id: "token.x".into(),
        })
    });
    let manifest = DataKitManifest::new().with_fragment(FragmentSourceEntry::emitter("tok", "tok"));
    let kit = DataKit::assemble(&manifest, &reg).unwrap();
    let s = format!("{:?}", kit);
    assert!(s.contains("DataKit"));
    assert!(s.contains("lang"));
    assert!(s.contains("runtime_config"));
}

#[test]
fn assemble_merges_ast_and_options_fragments() {
    let mut reg = EmitterRegistry::new();
    reg.register_emitter("ast-opt", || Box::new(AstOptionEmitter));
    let manifest =
        DataKitManifest::new().with_fragment(FragmentSourceEntry::emitter("ast-opt", "ast-opt"));
    let kit = DataKit::assemble(&manifest, &reg).unwrap();
    assert_eq!(kit.lang().ast.len(), 1);
    assert_eq!(kit.lang().options.len(), 1);
    assert!(kit.lang().ast.get_by_id("shape.x").is_some());
    assert!(kit.lang().options.get_by_id("opt.x").is_some());
}

#[test]
fn assemble_merges_caps_fragment() {
    let mut reg = EmitterRegistry::new();
    reg.register_emitter("cap", || Box::new(CapEmitter { cap_id: "cap.z" }));
    let manifest = DataKitManifest::new().with_fragment(FragmentSourceEntry::emitter("cap", "cap"));
    let kit = DataKit::assemble(&manifest, &reg).unwrap();
    assert_eq!(kit.lang().caps.len(), 1);
    assert!(kit.lang().caps.get_by_id("cap.z").is_some());
}

#[test]
fn exclude_skips_ast_and_options() {
    let mut reg = EmitterRegistry::new();
    reg.register_emitter("ast-opt", || Box::new(AstOptionEmitter));
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter("ast-opt", "ast-opt"))
        .with_exclude(vec!["shape.x".into(), "opt.x".into()]);
    let kit = DataKit::assemble(&manifest, &reg).unwrap();
    assert_eq!(kit.lang().ast.len(), 0);
    assert_eq!(kit.lang().options.len(), 0);
}

#[test]
fn exclude_skips_eval_and_caps_fragments() {
    // Exercising the "skip" branch of the include/exclude filter for the eval
    // and caps fragment sets (coverage of the `if passes(...)` false path).
    let mut reg = EmitterRegistry::new();
    reg.register_emitter("c", || {
        Box::new(ConstructEmitter {
            token_id: "token.c",
            grammar_id: "g.c",
            eval_id: "e.c",
            node_kind: "c",
        })
    });
    reg.register_emitter("cap", || Box::new(CapEmitter { cap_id: "cap.z" }));
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter("c", "c"))
        .with_fragment(FragmentSourceEntry::emitter("cap", "cap"))
        .with_exclude(vec!["e.c".into(), "cap.z".into()]);
    let kit = DataKit::assemble(&manifest, &reg).unwrap();
    // eval and caps fragments are excluded; token and grammar remain.
    assert_eq!(kit.lang().eval.len(), 0);
    assert_eq!(kit.lang().caps.len(), 0);
    assert_eq!(kit.lang().tokens.len(), 1);
    assert_eq!(kit.lang().grammar.len(), 1);
}
