//! Inline unit tests for `handl-kit-data` (ADR-0044 Phase 5 coverage).
//!
//! These cover: manifest parsing (serde round-trip), kit assembly (merge in
//! declared order), include/exclude filtering, the exclude-replaces
//! interaction (explicit error, ADR §5), the `extends` chain, and the
//! `base_language` entry point that lets a Rust-computed `Language` (e.g.
//! the canonical YAML language) enter kit assembly directly.

use super::*;
use handl_language::{
    AstShapeRef, CapabilityDecl, EvalSchemeDef, GrammarPosition, GrammarRow, LangError, Language,
    Term, TokenKindDef,
};
use handl_runtime::RuntimeConfig;
use serde_json::json;

// ---------------------------------------------------------------------------
// Language fixtures
// ---------------------------------------------------------------------------

/// A language holding the single token kind `id`.
fn token_language(id: &str) -> Language {
    let mut lang = Language::new();
    lang.tokens.merge(TokenKindDef::new(id)).unwrap();
    lang
}

/// A language holding only `token.A`.
fn token_a_language() -> Language {
    token_language("token.A")
}

/// A language holding only `token.A2` (with `replaces: ["token.A"]`).
///
/// Built by merging `token.A` then `token.A2` into one `Language`: because
/// `FragmentSet::merge` applies replaces destructively within a single
/// `Language`, the result contains only `A2` still carrying the replaces
/// declaration. When `A2` is later merged into an accumulator that already
/// has `A` (from an earlier data-file source), the cross-source replace
/// succeeds. This models the kit-level override: a later fragment source
/// overrides an earlier one.
fn token_aprime_language() -> Language {
    let mut lang = Language::new();
    lang.tokens.merge(TokenKindDef::new("token.A")).unwrap();
    lang.tokens
        .merge(TokenKindDef::new("token.A2").with_replaces(vec!["token.A".into()]))
        .unwrap();
    lang
}

/// A language holding a token + grammar + eval fragment (a minimal
/// construct).
fn construct_language(
    token_id: &str,
    grammar_id: &str,
    eval_id: &str,
    node_kind: &str,
) -> Language {
    let mut lang = Language::new();
    lang.tokens.merge(TokenKindDef::new(token_id)).unwrap();
    lang.grammar
        .merge(GrammarRow::new(
            grammar_id,
            token_id,
            GrammarPosition::Prefix,
            AstShapeRef::new(node_kind, 0),
        ))
        .unwrap();
    lang.eval
        .merge(EvalSchemeDef::new(
            eval_id,
            node_kind,
            serde_json::from_value::<Term>(json!({"Arr": {"value": null}})).unwrap(),
        ))
        .unwrap();
    lang
}

/// A language declaring a single capability.
fn cap_language(cap_id: &str) -> Language {
    let mut lang = Language::new();
    lang.caps.merge(CapabilityDecl::new(cap_id)).unwrap();
    lang
}

/// A language holding an AST shape and an option schema (exercises the `ast`
/// and `options` merge paths in `merge_filtered`).
fn ast_option_language() -> Language {
    let mut lang = Language::new();
    lang.ast
        .merge(handl_language::AstShapeDef::new("shape.x", "x"))
        .unwrap();
    lang.options
        .merge(handl_language::OptionSchema::new("opt.x"))
        .unwrap();
    lang
}

/// `(name, language)` pairs for the seven minimal constructs used by the
/// multi-source assembly test.
fn seven_construct_languages() -> Vec<(&'static str, Language)> {
    [
        (
            "identifier",
            "token.identifier",
            "grammar.identifier.prefix",
            "eval.var",
            "var",
        ),
        (
            "value",
            "token.number",
            "grammar.value.number.prefix",
            "eval.value.number",
            "value.number",
        ),
        (
            "arith",
            "token.plus",
            "grammar.add.infix",
            "eval.binary.add",
            "binary.add",
        ),
        (
            "let",
            "token.keyword.let",
            "grammar.let.prefix",
            "eval.let",
            "let",
        ),
        (
            "call",
            "token.lparen",
            "grammar.call.infix",
            "eval.call",
            "call",
        ),
        (
            "branch",
            "token.keyword.if",
            "grammar.if.prefix",
            "eval.if",
            "if",
        ),
        (
            "function",
            "token.arrow",
            "grammar.arrow.infix",
            "eval.function",
            "function",
        ),
    ]
    .into_iter()
    .map(|(name, tok, gram, eval, kind)| {
        (name, construct_language(tok, gram, eval, kind))
    })
    .collect()
}

/// Write `contents` to a unique temp file named `<name>.<ext>` and return its
/// path. The workspace has no `tempfile` dev-dependency; the convention is
/// `std::env::temp_dir()` plus the process id (see `handl-runtime/tests`).
fn temp_file(name: &str, ext: &str, contents: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("handl-kit-data-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.{ext}"));
    std::fs::write(&path, contents).unwrap();
    path
}

/// Serialize `lang` to a temp JSON data file and return its path — the
/// data-file form of a fragment source.
fn lang_file(name: &str, lang: &Language) -> std::path::PathBuf {
    temp_file(name, "json", &serde_json::to_string(lang).unwrap())
}

// ---------------------------------------------------------------------------
// Manifest parsing / serde round-trip
// ---------------------------------------------------------------------------

#[test]
fn manifest_serde_roundtrip_data_file_sources() {
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("arith", "lang/arith.json"))
        .with_fragment(FragmentSourceEntry::data_file("value", "lang/value.json"))
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
fn manifest_serde_roundtrip_single_data_file_source() {
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("data", "lang/data.toml"));
    let j = serde_json::to_string(&manifest).unwrap();
    let back: DataKitManifest = serde_json::from_str(&j).unwrap();
    assert_eq!(manifest, back);
    assert_eq!(back.fragments[0].source, PathBuf::from("lang/data.toml"));
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
fn fragment_source_entry_data_file_constructor() {
    let e = FragmentSourceEntry::data_file("name", "path.toml");
    assert_eq!(e.name, "name");
    assert_eq!(e.source, PathBuf::from("path.toml"));
}

// ---------------------------------------------------------------------------
// KitRegistry
// ---------------------------------------------------------------------------

#[test]
fn registry_resolves_kit_manifest() {
    let mut reg = KitRegistry::new();
    let manifest = DataKitManifest::new().with_extends("none");
    reg.register_kit("my-kit", manifest.clone());
    let resolved = reg.resolve_kit("my-kit").unwrap();
    assert_eq!(resolved.extends, Some("none".into()));
}

#[test]
fn registry_unknown_kit_returns_none() {
    let reg = KitRegistry::new();
    assert!(reg.resolve_kit("ghost").is_none());
}

#[test]
fn kit_registry_default_is_empty() {
    let reg = KitRegistry::default();
    assert!(reg.resolve_kit("anything").is_none());
}

// ---------------------------------------------------------------------------
// Kit assembly: merge fragments in declared order
// ---------------------------------------------------------------------------

#[test]
fn assemble_merges_fragments_in_declared_order() {
    let a = lang_file("order-a", &token_language("token.a"));
    let b = lang_file("order-b", &token_language("token.b"));
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("a", a))
        .with_fragment(FragmentSourceEntry::data_file("b", b));
    let kit = DataKit::assemble(&manifest, &reg, Language::new()).unwrap();
    assert_eq!(kit.lang().tokens.len(), 2);
    assert!(kit.lang().tokens.get_by_id("token.a").is_some());
    assert!(kit.lang().tokens.get_by_id("token.b").is_some());
}

#[test]
fn assemble_merges_onto_base_language() {
    // The canonical entry point: a caller that already holds a `Language`
    // passes it as `base_language`; the kit's data-file sources merge on top.
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new();
    let kit = DataKit::assemble(&manifest, &reg, token_a_language()).unwrap();
    assert_eq!(kit.lang().tokens.len(), 1);
    assert!(kit.lang().tokens.get_by_id("token.A").is_some());
}

#[test]
fn base_language_and_data_file_merge_together() {
    // A data file participates in the same accumulator as the base language:
    // the file's `token.x` and the base's `token.A` coexist.
    let path = lang_file("base-merge", &token_language("token.x"));
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("d", path.clone()));
    let kit = DataKit::assemble(&manifest, &reg, token_a_language()).unwrap();
    assert_eq!(kit.lang().tokens.len(), 2);
    let _ = std::fs::remove_file(path);
}

#[test]
fn assemble_produces_language_and_runtime_config() {
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new().with_runtime(
        RuntimeConfig::new()
            .with_parse("pratt")
            .with_eval("tree-walk"),
    );
    let kit = DataKit::assemble(&manifest, &reg, token_language("token.x")).unwrap();
    assert_eq!(kit.lang().tokens.len(), 1);
    assert_eq!(kit.runtime_config().parse, "pratt");
    assert_eq!(kit.runtime_config().eval, "tree-walk");
}

#[test]
fn assemble_construct_language_merges_all_three_sets() {
    let path = lang_file("construct", &construct_language("token.c", "g.c", "e.c", "c"));
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("c", path.clone()));
    let kit = DataKit::assemble(&manifest, &reg, Language::new()).unwrap();
    assert_eq!(kit.lang().tokens.len(), 1);
    assert_eq!(kit.lang().grammar.len(), 1);
    assert_eq!(kit.lang().eval.len(), 1);
    let _ = std::fs::remove_file(path);
}

// ---------------------------------------------------------------------------
// Include / exclude
// ---------------------------------------------------------------------------

#[test]
fn exclude_skips_fragment_by_id() {
    let a = lang_file("excl-a", &token_language("token.a"));
    let b = lang_file("excl-b", &token_language("token.b"));
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("a", a))
        .with_fragment(FragmentSourceEntry::data_file("b", b))
        .with_exclude(vec!["token.a".into()]);
    let kit = DataKit::assemble(&manifest, &reg, Language::new()).unwrap();
    assert_eq!(kit.lang().tokens.len(), 1);
    assert!(kit.lang().tokens.get_by_id("token.a").is_none());
    assert!(kit.lang().tokens.get_by_id("token.b").is_some());
}

#[test]
fn include_keeps_only_listed_ids() {
    let a = lang_file("incl-a", &token_language("token.a"));
    let b = lang_file("incl-b", &token_language("token.b"));
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("a", a))
        .with_fragment(FragmentSourceEntry::data_file("b", b))
        .with_include(vec!["token.a".into()]);
    let kit = DataKit::assemble(&manifest, &reg, Language::new()).unwrap();
    assert_eq!(kit.lang().tokens.len(), 1);
    assert!(kit.lang().tokens.get_by_id("token.a").is_some());
    assert!(kit.lang().tokens.get_by_id("token.b").is_none());
}

#[test]
fn include_empty_keeps_all() {
    let a = lang_file("incl-empty", &token_language("token.a"));
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("a", a));
    let kit = DataKit::assemble(&manifest, &reg, Language::new()).unwrap();
    assert_eq!(kit.lang().tokens.len(), 1);
}

#[test]
fn exclude_takes_precedence_over_include() {
    // A fragment in both include and exclude is excluded.
    let a = lang_file("incl-excl", &token_language("token.a"));
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("a", a))
        .with_include(vec!["token.a".into()])
        .with_exclude(vec!["token.a".into()]);
    let kit = DataKit::assemble(&manifest, &reg, Language::new()).unwrap();
    assert_eq!(kit.lang().tokens.len(), 0);
}

#[test]
fn exclude_does_not_filter_base_language() {
    // The include/exclude filter operates on fragment *sources*; the base
    // language is the accumulator itself and merges unconditionally.
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new().with_exclude(vec!["token.A".into()]);
    let kit = DataKit::assemble(&manifest, &reg, token_a_language()).unwrap();
    assert!(kit.lang().tokens.get_by_id("token.A").is_some());
}

#[test]
fn exclude_across_multiple_sets() {
    // Excluding a grammar id skips only that grammar row, not the token.
    let path = lang_file("excl-multi", &construct_language("token.c", "g.c", "e.c", "c"));
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("c", path.clone()))
        .with_exclude(vec!["g.c".into()]);
    let kit = DataKit::assemble(&manifest, &reg, Language::new()).unwrap();
    assert_eq!(kit.lang().tokens.len(), 1);
    assert_eq!(kit.lang().grammar.len(), 0);
    assert_eq!(kit.lang().eval.len(), 1);
    let _ = std::fs::remove_file(path);
}

// ---------------------------------------------------------------------------
// Exclude a fragment that others `replaces` → explicit error (ADR §5)
// ---------------------------------------------------------------------------

#[test]
fn exclude_replaces_target_errors_explicitly() {
    // Scenario (ADR §5): source1 carries `token.A`; source2 carries `token.A2`
    // which `replaces: ["token.A"]`. Excluding `token.A` means A is never
    // merged into the accumulator, so when A2 is merged its replace target is
    // absent → `UnknownReplace` error. This is the explicit, non-silent
    // behavior the ADR requires.
    let a = lang_file("repl-a", &token_a_language());
    let aprime = lang_file("repl-aprime", &token_aprime_language());
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("a", a))
        .with_fragment(FragmentSourceEntry::data_file("aprime", aprime))
        .with_exclude(vec!["token.A".into()]);
    let err = DataKit::assemble(&manifest, &reg, Language::new()).unwrap_err();
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
    let a = lang_file("repl2-a", &token_a_language());
    let aprime = lang_file("repl2-aprime", &token_aprime_language());
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("a", a))
        .with_fragment(FragmentSourceEntry::data_file("aprime", aprime));
    let kit = DataKit::assemble(&manifest, &reg, Language::new()).unwrap();
    assert_eq!(kit.lang().tokens.len(), 1);
    assert!(kit.lang().tokens.get_by_id("token.A").is_none());
    assert!(kit.lang().tokens.get_by_id("token.A2").is_some());
}

// ---------------------------------------------------------------------------
// `extends` chain
// ---------------------------------------------------------------------------

#[test]
fn extends_assembles_parent_then_child() {
    let parent_lang = lang_file("parent-tok", &token_language("token.parent"));
    let child_lang = lang_file("child-tok", &token_language("token.child"));
    let mut reg = KitRegistry::new();
    let parent = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("parent-tok", parent_lang));
    reg.register_kit("parent-kit", parent);
    let child = DataKitManifest::new()
        .with_extends("parent-kit")
        .with_fragment(FragmentSourceEntry::data_file("child-tok", child_lang));
    let kit = DataKit::assemble(&child, &reg, Language::new()).unwrap();
    assert_eq!(kit.lang().tokens.len(), 2);
    assert!(kit.lang().tokens.get_by_id("token.parent").is_some());
    assert!(kit.lang().tokens.get_by_id("token.child").is_some());
}

#[test]
fn extends_threads_base_language_to_chain_root() {
    // The caller-supplied `base_language` lands at the root of the `extends`
    // chain: a child with no fragments and a parent with no fragments still
    // yields the base language.
    let mut reg = KitRegistry::new();
    reg.register_kit("parent-kit", DataKitManifest::new());
    let child = DataKitManifest::new().with_extends("parent-kit");
    let kit = DataKit::assemble(&child, &reg, token_a_language()).unwrap();
    assert_eq!(kit.lang().tokens.len(), 1);
    assert!(kit.lang().tokens.get_by_id("token.A").is_some());
}

#[test]
fn extends_base_and_child_fragments_merge() {
    // Parent is empty; the base lands at the root, then the child's own
    // data-file source merges on top.
    let child_lang = lang_file("child-merge", &token_language("token.child"));
    let mut reg = KitRegistry::new();
    reg.register_kit("parent-kit", DataKitManifest::new());
    let child = DataKitManifest::new()
        .with_extends("parent-kit")
        .with_fragment(FragmentSourceEntry::data_file("child", child_lang));
    let kit = DataKit::assemble(&child, &reg, token_a_language()).unwrap();
    assert_eq!(kit.lang().tokens.len(), 2);
    assert!(kit.lang().tokens.get_by_id("token.A").is_some());
    assert!(kit.lang().tokens.get_by_id("token.child").is_some());
}

#[test]
fn extends_unknown_parent_errors() {
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new().with_extends("ghost-kit");
    let err = DataKit::assemble(&manifest, &reg, Language::new()).unwrap_err();
    assert_eq!(err, LangError::engine("unknown parent kit: ghost-kit"));
}

#[test]
fn extends_child_overrides_parent_fragment() {
    // Parent declares token.X. Child's source carries only Y (with
    // replaces:["token.X"]) — the same within-language merge shape as
    // `token_aprime_language`. When Y merges into the accumulator (which has
    // X from the parent), Y replaces X — the cross-kit override.
    let parent_lang = lang_file("parent-x", &token_language("token.X"));
    let mut lang = Language::new();
    lang.tokens.merge(TokenKindDef::new("token.X")).unwrap();
    lang.tokens
        .merge(TokenKindDef::new("token.Y").with_replaces(vec!["token.X".into()]))
        .unwrap();
    let child_lang = lang_file("child-y", &lang);
    let mut reg = KitRegistry::new();
    let parent = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("x", parent_lang));
    reg.register_kit("parent-kit", parent);
    let child = DataKitManifest::new()
        .with_extends("parent-kit")
        .with_fragment(FragmentSourceEntry::data_file("replace-x", child_lang));
    let kit = DataKit::assemble(&child, &reg, Language::new()).unwrap();
    assert!(kit.lang().tokens.get_by_id("token.X").is_none());
    assert!(kit.lang().tokens.get_by_id("token.Y").is_some());
    assert_eq!(kit.lang().tokens.len(), 1);
}

#[test]
fn extends_chain_union_of_capabilities() {
    // Parent declares cap.a; child declares cap.b → union.
    let parent_lang = lang_file("cap-a", &cap_language("cap.a"));
    let child_lang = lang_file("cap-b", &cap_language("cap.b"));
    let mut reg = KitRegistry::new();
    let parent = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("cap-a", parent_lang));
    reg.register_kit("parent-kit", parent);
    let child = DataKitManifest::new()
        .with_extends("parent-kit")
        .with_fragment(FragmentSourceEntry::data_file("cap-b", child_lang));
    let kit = DataKit::assemble(&child, &reg, Language::new()).unwrap();
    assert_eq!(kit.lang().caps.len(), 2);
    assert!(kit.lang().caps.get_by_id("cap.a").is_some());
    assert!(kit.lang().caps.get_by_id("cap.b").is_some());
}

// ---------------------------------------------------------------------------
// Data-file source (ADR-0044 §6 end state)
// ---------------------------------------------------------------------------

/// A one-token language, used as the round-trip fixture.
fn sample_language() -> Language {
    token_language("token.x")
}

/// The minimal TOML document that deserializes to an *empty* [`Language`]:
/// the nine non-defaulted fragment sets as empty inline tables plus the
/// required `scheme_version`. (`ops`/`hands`/`lints`/… carry `serde(default)`.)
fn empty_language_toml() -> String {
    let mut s = String::from("scheme_version = \"v2\"\n");
    for set in [
        "tokens",
        "grammar",
        "eval",
        "types",
        "ast",
        "transforms",
        "caps",
        "options",
        "lexer_rules",
    ] {
        s.push_str(&format!(
            "\n[{set}]\nby_key = {{}}\nid_index = {{}}\noverride_chain = []\n"
        ));
    }
    s
}

#[test]
fn data_file_json_source_loads_language() {
    let lang = sample_language();
    let path = temp_file("lang", "json", &serde_json::to_string(&lang).unwrap());
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("d", path.clone()));
    let kit = DataKit::assemble(&manifest, &reg, Language::new()).unwrap();
    assert_eq!(kit.lang().tokens.len(), 1);
    let _ = std::fs::remove_file(path);
}

#[test]
fn data_file_yaml_source_loads_language() {
    let lang = sample_language();
    let path = temp_file("lang", "yaml", &serde_yaml::to_string(&lang).unwrap());
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("d", path.clone()));
    let kit = DataKit::assemble(&manifest, &reg, Language::new()).unwrap();
    assert_eq!(kit.lang().tokens.len(), 1);
    let _ = std::fs::remove_file(path);
}

#[test]
fn data_file_toml_source_loads_language() {
    let path = temp_file("empty", "toml", &empty_language_toml());
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("d", path.clone()));
    let kit = DataKit::assemble(&manifest, &reg, Language::new()).unwrap();
    assert_eq!(kit.lang().tokens.len(), 0);
    let _ = std::fs::remove_file(path);
}

#[test]
fn data_file_unknown_extension_falls_back_to_json() {
    let lang = sample_language();
    let path = temp_file("lang", "fragment", &serde_json::to_string(&lang).unwrap());
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("d", path.clone()));
    let kit = DataKit::assemble(&manifest, &reg, Language::new()).unwrap();
    assert_eq!(kit.lang().tokens.len(), 1);
    let _ = std::fs::remove_file(path);
}

#[test]
fn data_file_missing_file_errors_with_path() {
    let missing = std::env::temp_dir().join("handl-kit-data-does-not-exist.json");
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("d", missing.clone()));
    let err = DataKit::assemble(&manifest, &reg, Language::new()).unwrap_err();
    assert!(matches!(err, LangError::Engine { .. }));
    assert!(
        format!("{err}").contains("handl-kit-data-does-not-exist.json"),
        "error should name the file: {err}"
    );
}

#[test]
fn data_file_unparseable_errors_with_path() {
    let path = temp_file("garbage", "json", "{ not json");
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("d", path.clone()));
    let err = DataKit::assemble(&manifest, &reg, Language::new()).unwrap_err();
    assert!(matches!(err, LangError::Engine { .. }));
    assert!(format!("{err}").contains("garbage.json"), "{err}");
    let _ = std::fs::remove_file(path);
}

// ---------------------------------------------------------------------------
// Kit-manifest data files (`DataKitManifest::load`)
// ---------------------------------------------------------------------------

#[test]
fn kit_manifest_loads_from_yaml_file() {
    let manifest = DataKitManifest::new().with_extends("canonical");
    let path = temp_file("kit", "yaml", &serde_yaml::to_string(&manifest).unwrap());
    let back = DataKitManifest::load(&path).unwrap();
    assert_eq!(back, manifest);
    let _ = std::fs::remove_file(path);
}

#[test]
fn kit_manifest_load_from_text_yaml() {
    let m = DataKitManifest::load_from_text("extends: canonical\n", "yaml").unwrap();
    assert_eq!(m.extends, Some("canonical".into()));
}

#[test]
fn kit_manifest_load_missing_file_errors_with_path() {
    let missing = std::env::temp_dir().join("handl-kit-data-no-such-kit.yaml");
    let err = DataKitManifest::load(&missing).unwrap_err();
    assert!(matches!(err, LangError::Engine { .. }));
    assert!(format!("{err}").contains("handl-kit-data-no-such-kit.yaml"));
}

// ---------------------------------------------------------------------------
// into_runtime: engine selection from data
// ---------------------------------------------------------------------------

#[test]
fn into_runtime_binds_pratt_and_tree_walk() {
    use hnl_engine_exec_tree_walk::TreeWalkEngine;
    use hnl_engine_lex_pattern::DataLexerEngine;
    use hnl_engine_parse_pratt::PrattEngine;
    use handl_runtime::EngineRegistry;
    use std::rc::Rc;

    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new().with_runtime(
        RuntimeConfig::new()
            .with_parse("pratt")
            .with_eval("tree-walk"),
    );
    let kit = DataKit::assemble(&manifest, &reg, token_language("token.x")).unwrap();
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
    use hnl_engine_exec_tree_walk::TreeWalkEngine;
    use hnl_engine_lex_pattern::DataLexerEngine;
    use hnl_engine_parse_pratt::PrattEngine;
    use handl_runtime::EngineRegistry;
    use std::rc::Rc;

    let reg = KitRegistry::new();
    let manifest =
        DataKitManifest::new().with_runtime(RuntimeConfig::new().with_eval("nonsense"));
    let kit = DataKit::assemble(&manifest, &reg, token_language("token.x")).unwrap();
    let mut engine_reg = EngineRegistry::new();
    engine_reg.register_lex("data", Box::new(DataLexerEngine::new()));
    engine_reg.register_parse("pratt", PrattEngine::new());
    engine_reg.register_eval("tree-walk", Box::new(TreeWalkEngine::new()));
    let engine_reg = Rc::new(engine_reg);
    let err = kit.into_runtime(&engine_reg, None).unwrap_err();
    assert_eq!(err, LangError::engine("unknown eval engine: nonsense"));
}

// ---------------------------------------------------------------------------
// Full seven-source assembly (representative subset)
// ---------------------------------------------------------------------------

#[test]
fn assemble_seven_sources_produces_expected_counts() {
    let mut manifest = DataKitManifest::new().with_runtime(
        RuntimeConfig::new()
            .with_parse("pratt")
            .with_eval("tree-walk"),
    );
    for (name, lang) in seven_construct_languages() {
        let path = lang_file(name, &lang);
        manifest = manifest.with_fragment(FragmentSourceEntry::data_file(name, path));
    }
    let reg = KitRegistry::new();
    let kit = DataKit::assemble(&manifest, &reg, Language::new()).unwrap();
    // 7 sources contribute tokens, grammar rows, and eval schemes. The
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
fn fragment_source_entry_debug_shows_path() {
    let e = FragmentSourceEntry::data_file("name", "x.toml");
    let s = format!("{:?}", e);
    assert!(s.contains("x.toml"));
}

#[test]
fn data_kit_debug_formats() {
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new();
    let kit = DataKit::assemble(&manifest, &reg, token_language("token.x")).unwrap();
    let s = format!("{:?}", kit);
    assert!(s.contains("DataKit"));
    assert!(s.contains("lang"));
    assert!(s.contains("runtime_config"));
}

#[test]
fn assemble_merges_ast_and_options_fragments() {
    let path = lang_file("ast-opt", &ast_option_language());
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("ast-opt", path.clone()));
    let kit = DataKit::assemble(&manifest, &reg, Language::new()).unwrap();
    assert_eq!(kit.lang().ast.len(), 1);
    assert_eq!(kit.lang().options.len(), 1);
    assert!(kit.lang().ast.get_by_id("shape.x").is_some());
    assert!(kit.lang().options.get_by_id("opt.x").is_some());
    let _ = std::fs::remove_file(path);
}

#[test]
fn assemble_merges_caps_fragment() {
    let path = lang_file("cap-z", &cap_language("cap.z"));
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("cap", path.clone()));
    let kit = DataKit::assemble(&manifest, &reg, Language::new()).unwrap();
    assert_eq!(kit.lang().caps.len(), 1);
    assert!(kit.lang().caps.get_by_id("cap.z").is_some());
    let _ = std::fs::remove_file(path);
}

#[test]
fn exclude_skips_ast_and_options() {
    let path = lang_file("ast-opt-excl", &ast_option_language());
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("ast-opt", path.clone()))
        .with_exclude(vec!["shape.x".into(), "opt.x".into()]);
    let kit = DataKit::assemble(&manifest, &reg, Language::new()).unwrap();
    assert_eq!(kit.lang().ast.len(), 0);
    assert_eq!(kit.lang().options.len(), 0);
    let _ = std::fs::remove_file(path);
}

#[test]
fn exclude_skips_eval_and_caps_fragments() {
    // Exercising the "skip" branch of the include/exclude filter for the eval
    // and caps fragment sets (coverage of the `if passes(...)` false path).
    let construct = lang_file("excl-construct", &construct_language("token.c", "g.c", "e.c", "c"));
    let cap = lang_file("excl-cap", &cap_language("cap.z"));
    let reg = KitRegistry::new();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::data_file("c", construct))
        .with_fragment(FragmentSourceEntry::data_file("cap", cap))
        .with_exclude(vec!["e.c".into(), "cap.z".into()]);
    let kit = DataKit::assemble(&manifest, &reg, Language::new()).unwrap();
    // eval and caps fragments are excluded; token and grammar remain.
    assert_eq!(kit.lang().eval.len(), 0);
    assert_eq!(kit.lang().caps.len(), 0);
    assert_eq!(kit.lang().tokens.len(), 1);
    assert_eq!(kit.lang().grammar.len(), 1);
}
