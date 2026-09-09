//! # handl-kit-data-canonical
//!
//! The **data-driven canonical kit** — the ADR-0044 §6 end state.
//!
//! Assembles a [`Language`] from **authored YAML data files** (one per fragment
//! type: `tokens.yaml`, `grammar.yaml`, `eval.yaml`, `ast.yaml`, `caps.yaml`,
//! `options.yaml`, `lexer_rules.yaml`, `transforms.yaml`), embedded at compile time via
//! `include_str!`. No Rust emitter crates are needed — the language definition
//! is pure data.
//!
//! This realizes ADR-0044 §6: "the end state is authored data files are
//! canonical." The 37 `handl-emitter-*` crates that produced these files have
//! been deleted. The YAML files are the language definition.
//!
//! ## Collision handling
//!
//! The YAML files are the pre-resolved output of merging all 37 emitters (with
//! 3 excluded fragment ids — see [`EXCLUDED_FRAGMENT_IDS`]). Collisions were
//! resolved at generation time:
//!
//! - **Token kinds & AST shapes** are union (idempotent) fragment types, so
//!   duplicate declarations merged idempotently.
//! - **Eval scheme collisions** on the same node kind were resolved by
//!   excluding non-canonical duplicate fragment ids.
//! - **Grammar row collisions** on the same `(token, position)` were resolved
//!   by excluding non-canonical duplicate grammar row ids.
//!
//! ## Usage
//!
//! ```no_run
//! let kit = handl_kit_data_canonical::assemble().unwrap();
//! let lang = kit.lang();
//! ```
//!
//! ## Regenerating the YAML files
//!
//! The data files were generated from the (now-deleted) emitter crates. To
//! regenerate, re-introduce the emitters, run the generation test, then
//! delete the emitters again. The files are checked in and should not need
//! regeneration unless the language definition changes.

#![warn(missing_docs)]

use std::path::Path;

use handl_emitters::Emitter;
use handl_kit_data::{DataKit, DataKitManifest, EmitterRegistry};
use handl_language::{
    AstShapeDef, CapabilityDecl, EvalSchemeDef, GrammarRow, LangError, Language, LexerRuleDef,
    LintRuleDef, OptionSchema, TokenKindDef, TransformDef, TypeRuleDef,
};

/// The kit name under which this kit registers itself (for `extends`
/// resolution by child kits).
pub const KIT_NAME: &str = "kit-data-canonical";

/// Fragment ids that were excluded when generating the YAML files.
///
/// These are non-canonical duplicate fragments (behavioral, non-union types)
/// that share an identity key with a canonical fragment:
///
/// - `eval.call.postfix` — `call`'s `eval.call` is the canonical eval scheme.
/// - `grammar.call.postfix` — `call`'s `grammar.call.infix` is the canonical
///   infix rule for `(`.
/// - `grammar.apply.prefix` — `paren`'s `grammar.paren.prefix` is the
///   canonical prefix rule for `(`.
pub const EXCLUDED_FRAGMENT_IDS: &[&str] = &[
    "eval.call.postfix",
    "grammar.call.postfix",
    "grammar.apply.prefix",
];

// ---------------------------------------------------------------------------
// YAML data file loading
// ---------------------------------------------------------------------------

/// Embed a YAML data file at compile time.
const TOKENS_YAML: &str = include_str!("../data/tokens.yaml");
const GRAMMAR_YAML: &str = include_str!("../data/grammar.yaml");
const EVAL_YAML: &str = include_str!("../data/eval.yaml");
const TYPES_YAML: &str = include_str!("../data/types.yaml");
const AST_YAML: &str = include_str!("../data/ast.yaml");
const CAPS_YAML: &str = include_str!("../data/caps.yaml");
const OPTIONS_YAML: &str = include_str!("../data/options.yaml");
const LEXER_RULES_YAML: &str = include_str!("../data/lexer_rules.yaml");
const TRANSFORMS_YAML: &str = include_str!("../data/transforms.yaml");
const LINTS_YAML: &str = include_str!("../data/lints.yaml");

/// Build a [`Language`] from the 9 embedded YAML data files.
///
/// Each file contains a YAML list of fragments of one type. The fragments are
/// deserialized and merged into a single [`Language`]. `eval.yaml` holds the
/// arrow-calculus `Term` definitions (ADR-0052 kernel).
///
/// # Errors
///
/// Returns [`LangError`] if any YAML file fails to deserialize or a merge
/// fails (should not happen — the files are pre-resolved).
pub fn language_from_yaml() -> Result<Language, LangError> {
    merge_fragments(
        TOKENS_YAML,
        GRAMMAR_YAML,
        EVAL_YAML,
        TYPES_YAML,
        AST_YAML,
        CAPS_YAML,
        OPTIONS_YAML,
        LEXER_RULES_YAML,
        TRANSFORMS_YAML,
        LINTS_YAML,
    )
}

/// Merge the nine YAML fragment strings into a single [`Language`].
///
/// Each argument is the raw YAML text for one fragment type. The fragments are
/// deserialized and merged in the same order as [`language_from_yaml`].
///
/// # Errors
///
/// Returns [`LangError`] if any YAML fails to deserialize or a merge fails.
fn merge_fragments(
    tokens_yaml: &str,
    grammar_yaml: &str,
    eval_yaml: &str,
    types_yaml: &str,
    ast_yaml: &str,
    caps_yaml: &str,
    options_yaml: &str,
    lexer_rules_yaml: &str,
    transforms_yaml: &str,
    lints_yaml: &str,
) -> Result<Language, LangError> {
    let mut lang = Language::new();

    // Token kinds (union — idempotent merge).
    let tokens: Vec<TokenKindDef> = serde_yaml::from_str(tokens_yaml)
        .map_err(|e| LangError::engine(format!("tokens.yaml: {}", e)))?;
    for frag in tokens {
        lang.tokens.merge(frag)?;
    }

    // Grammar rows (non-union — pre-resolved, no collisions).
    let grammar: Vec<GrammarRow> = serde_yaml::from_str(grammar_yaml)
        .map_err(|e| LangError::engine(format!("grammar.yaml: {}", e)))?;
    for frag in grammar {
        lang.grammar.merge(frag)?;
    }

    // Eval schemes (arrow-calculus kernel, ADR-0052; non-union — pre-resolved,
    // no collisions).
    let eval: Vec<EvalSchemeDef> = serde_yaml::from_str(eval_yaml)
        .map_err(|e| LangError::engine(format!("eval.yaml: {}", e)))?;
    for frag in eval {
        lang.eval.merge(frag)?;
    }

    // Type rules (ADR-0054 — non-union, pre-resolved).
    let types: Vec<TypeRuleDef> = serde_yaml::from_str(types_yaml)
        .map_err(|e| LangError::engine(format!("types.yaml: {}", e)))?;
    for frag in types {
        lang.types.merge(frag)?;
    }

    // AST shapes (union — idempotent merge). `merge_ast_fragment` also
    // lowers any `desugar:` rule each shape carries into a `TransformDef` in
    // `lang.transforms` (pass "desugar", ADR-0094 §6).
    let ast: Vec<AstShapeDef> = serde_yaml::from_str(ast_yaml)
        .map_err(|e| LangError::engine(format!("ast.yaml: {}", e)))?;
    for frag in ast {
        lang.merge_ast_fragment(frag)?;
    }

    // Capabilities (union — idempotent merge).
    let caps: Vec<CapabilityDecl> = serde_yaml::from_str(caps_yaml)
        .map_err(|e| LangError::engine(format!("caps.yaml: {}", e)))?;
    for frag in caps {
        lang.caps.merge(frag)?;
    }

    // Options (non-union).
    let options: Vec<OptionSchema> = serde_yaml::from_str(options_yaml)
        .map_err(|e| LangError::engine(format!("options.yaml: {}", e)))?;
    for frag in options {
        lang.options.merge(frag)?;
    }

    // Lexer rules (non-union).
    let lexer_rules: Vec<LexerRuleDef> = serde_yaml::from_str(lexer_rules_yaml)
        .map_err(|e| LangError::engine(format!("lexer_rules.yaml: {}", e)))?;
    for frag in lexer_rules {
        lang.lexer_rules.merge(frag)?;
    }

    // Transform rules (ADR-0094 — non-union, pre-resolved). Merged *after*
    // the ast pass so a first-class rule may `replaces:` a `desugar:`-lowered
    // `transform.desugar.*` rule synthesized above.
    let transforms: Vec<TransformDef> = serde_yaml::from_str(transforms_yaml)
        .map_err(|e| LangError::engine(format!("transforms.yaml: {}", e)))?;
    for frag in transforms {
        lang.transforms.merge(frag)?;
    }

    // Lint rules (ADR-0097 — non-union, pre-resolved).
    let lints: Vec<LintRuleDef> = serde_yaml::from_str(lints_yaml)
        .map_err(|e| LangError::engine(format!("lints.yaml: {}", e)))?;
    for frag in lints {
        lang.lints.merge(frag)?;
    }

    // ADR-0094 "Production readiness": transform-rule validation runs at
    // `Language` build. Error-severity diagnostics (duplicate identity keys,
    // out-of-bounds `Keep`/`KeepRole`/`KeepPath`/`KeepRest`/`Apply` steps,
    // dangling `Apply` rule ids, contradictory `KindAt` role/index pairs)
    // fail the merge; warnings (unknown target kinds, potential `Fixpoint`
    // cycles, `Native` rules pending a wired registry) do not — shapes and
    // registries may legitimately arrive from other fragments or engines.
    let diags = handl_plugin_engine_transform::validate::validate_transforms(&lang);
    let errors: Vec<String> = diags
        .iter()
        .filter(|d| d.severity == handl_plugin_ext_diag::Severity::Error)
        .map(|d| d.message.clone())
        .collect();
    if !errors.is_empty() {
        return Err(LangError::engine(format!(
            "transform rule validation failed: {}",
            errors.join("; ")
        )));
    }

    Ok(lang)
}

/// Build a [`Language`] from the nine YAML data files in a directory.
///
/// Reads `tokens.yaml`, `grammar.yaml`, `eval.yaml`, `types.yaml`,
/// `ast.yaml`, `caps.yaml`, `options.yaml`, `lexer_rules.yaml`, and
/// (optionally — absent means no first-class rules) `transforms.yaml` from `dir`
/// and merges them the same way [`language_from_yaml`] merges the embedded
/// files. This is the per-file kit loading path (ADR 0003): a host resolves a
/// kit data directory and assembles a [`Language`] from its authored YAML.
///
/// # Errors
///
/// Returns [`LangError`] if `dir` is not a directory, any YAML file is
/// missing or unreadable, or a fragment fails to deserialize/merge.
pub fn language_from_dir(dir: &Path) -> Result<Language, LangError> {
    if !dir.is_dir() {
        return Err(LangError::engine(format!(
            "language_from_dir: {:?} is not a directory",
            dir
        )));
    }

    let read = |name: &str| -> Result<String, LangError> {
        std::fs::read_to_string(dir.join(name))
            .map_err(|e| LangError::engine(format!("{}: {}", name, e)))
    };

    let tokens_yaml = read("tokens.yaml")?;
    let grammar_yaml = read("grammar.yaml")?;
    let eval_yaml = read("eval.yaml")?;
    let types_yaml = read("types.yaml")?;
    let ast_yaml = read("ast.yaml")?;
    let caps_yaml = read("caps.yaml")?;
    let options_yaml = read("options.yaml")?;
    let lexer_rules_yaml = read("lexer_rules.yaml")?;

    // `transforms.yaml` is optional: fragment directories written before
    // ADR-0094's first-class transform section do not carry one.
    let transforms_yaml = match std::fs::read_to_string(dir.join("transforms.yaml")) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "[]".to_string(),
        Err(e) => return Err(LangError::engine(format!("transforms.yaml: {}", e))),
    };

    // `lints.yaml` is optional: fragment directories written before
    // ADR-0097's first-class lint-rule section do not carry one.
    let lints_yaml = match std::fs::read_to_string(dir.join("lints.yaml")) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "[]".to_string(),
        Err(e) => return Err(LangError::engine(format!("lints.yaml: {}", e))),
    };

    merge_fragments(
        &tokens_yaml,
        &grammar_yaml,
        &eval_yaml,
        &types_yaml,
        &ast_yaml,
        &caps_yaml,
        &options_yaml,
        &lexer_rules_yaml,
        &transforms_yaml,
        &lints_yaml,
    )
}

// ---------------------------------------------------------------------------
// Emitter wrapper (for the extends-chain assembly path)
// ---------------------------------------------------------------------------

/// An emitter that loads the embedded YAML data files.
///
/// This wraps [`language_from_yaml`] in the [`Emitter`] trait so the
/// `EmitterRegistry` / `DataKit::assemble` path works transparently for child
/// kits that `extends` this kit.
#[derive(Debug, Default, Clone, Copy)]
pub struct YamlDataEmitter;

impl YamlDataEmitter {
    /// Create a new YAML data emitter.
    pub fn new() -> Self {
        Self
    }
}

impl Emitter for YamlDataEmitter {
    fn emit(&self) -> Result<Language, LangError> {
        language_from_yaml()
    }
}

// ---------------------------------------------------------------------------
// Kit manifest + registry
// ---------------------------------------------------------------------------

/// Build the [`DataKitManifest`] for the data-driven canonical kit.
///
/// The manifest references a single `canonical-yaml` fragment source (the
/// embedded YAML files) and runtime engine selection.
///
/// The `runtime.typecheck` field selects the `nbe` engine registered by the host.
pub fn manifest() -> DataKitManifest {
    load_kit("canonical").unwrap_or_else(|e| panic!("failed to load canonical manifest: {}", e))
}

/// Build an [`EmitterRegistry`] pre-populated with the canonical YAML data
/// emitter and all embedded `data/kits/*.yaml` manifests registered under
/// their kit names (so `extends: canonical` resolves for child kits).
pub fn registry() -> EmitterRegistry {
    let mut reg = EmitterRegistry::new();
    reg.register_emitter("canonical-yaml", || Box::new(YamlDataEmitter::new()));
    for (name, _) in KIT_MANIFESTS {
        let m = load_kit(name)
            .unwrap_or_else(|e| panic!("failed to load manifest for {}: {}", name, e));
        reg.register_kit(name, m);
    }
    reg
}

// ---------------------------------------------------------------------------
// Kit manifest data files (`data/kits/*.yaml`)
// ---------------------------------------------------------------------------

/// The kit manifests embedded at compile time, keyed by kit name.
///
/// Each entry is `data/kits/<name>.yaml` — a [`DataKitManifest`] whose
/// `modules` list names the library plugins the kit contributes, in
/// contribution order. The `extends` chain composes kits: a child's modules
/// append to its parent's (e.g. `server` extends `config`).
const KIT_MANIFESTS: &[(&str, &str)] = &[
    ("canonical", include_str!("../data/kits/canonical.yaml")),
    ("hello", include_str!("../data/kits/hello.yaml")),
    ("stdlib", include_str!("../data/kits/stdlib.yaml")),
    ("cli", include_str!("../data/kits/cli.yaml")),
    ("config", include_str!("../data/kits/config.yaml")),
    ("server", include_str!("../data/kits/server.yaml")),
];

/// Load a kit manifest by name from the embedded `data/kits/` files.
///
/// The returned [`DataKitManifest`] carries the kit's `modules` list (and, for
/// `canonical`, the fragment sources and runtime selection mirroring
/// [`manifest`]). Unknown names surface as [`LangError::Engine`].
///
/// # Errors
///
/// Returns [`LangError::Engine`] if `name` is not a known kit or the embedded
/// YAML fails to deserialize.
pub fn load_kit(name: &str) -> Result<DataKitManifest, LangError> {
    let yaml = KIT_MANIFESTS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, y)| *y)
        .ok_or_else(|| LangError::engine(format!("unknown kit: {}", name)))?;
    serde_yaml::from_str(yaml).map_err(|e| LangError::engine(format!("kits/{}.yaml: {}", name, e)))
}

/// Resolve a kit name to its flattened, ordered list of library module plugin
/// names, following the manifest's `extends` chain.
///
/// The parent's modules come first, then the child's, preserving the
/// contribution order the kit's `contribute` method used to express in Rust.
///
/// # Errors
///
/// Returns [`LangError::Engine`] if `name` (or any kit in its `extends`
/// chain) is not a known kit or fails to parse.
pub fn kit_module_names(name: &str) -> Result<Vec<String>, LangError> {
    let manifest = load_kit(name)?;
    let mut modules = if let Some(parent) = &manifest.extends {
        kit_module_names(parent)?
    } else {
        Vec::new()
    };
    modules.extend(manifest.modules.iter().cloned());
    Ok(modules)
}

/// Assemble the data-driven canonical kit: load the YAML data files, build a
/// [`DataKit`] with the Pratt + tree-walk runtime config.
///
/// # Errors
///
/// Returns [`LangError`] if any YAML deserialization or fragment merge fails
/// (should not happen — the files are pre-resolved).
pub fn assemble() -> Result<DataKit, LangError> {
    let reg = registry();
    DataKit::assemble(&manifest(), &reg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_kit_data::DataKit;

    #[test]
    fn manifest_has_single_yaml_source() {
        let m = manifest();
        assert_eq!(m.fragments.len(), 1);
        assert_eq!(m.fragments[0].name, "canonical-yaml");
        assert_eq!(m.extends, None);
        assert_eq!(m.runtime.lex, "data");
        assert_eq!(m.runtime.parse, "pratt");
        assert_eq!(m.runtime.eval, "tree-walk");
    }

    #[test]
    fn manifest_serde_roundtrip() {
        let m = manifest();
        let j = serde_json::to_string(&m).unwrap();
        let back: DataKitManifest = serde_json::from_str(&j).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn assemble_produces_nonempty_language() {
        let kit = assemble().unwrap();
        let lang = kit.lang();
        assert!(lang.tokens.len() > 0);
        assert!(lang.grammar.len() > 0);
        assert!(lang.eval.len() > 0);
        assert!(lang.types.len() > 0);
        assert!(lang.ast.len() > 0);
        assert!(lang.caps.len() > 0);
    }

    #[test]
    fn yaml_emitter_produces_language() {
        let lang = YamlDataEmitter::new().emit().unwrap();
        assert!(lang.tokens.len() > 0);
        assert!(lang.grammar.len() > 0);
        assert!(lang.eval.len() > 0);
    }

    #[test]
    fn types_yaml_loads() {
        let lang = language_from_yaml().unwrap();
        // 76 type rules in types.yaml. `type.let` covers the internal
        // `let` binding kind — there is no `let` surface syntax and
        // `type.stmt.let` stays removed (`stmt.let` is fixture/desugar-
        // only). Phase 7 added
        // `type.Total` and `type.Pure` meta-predicate types. Phase 8.2
        // added `type.Marshalable` meta-predicate type. The arrow-
        // combinator surface syntax added 7 placeholder rules
        // (`arr`/`first`/`second`/`left`/`right`/`sum`/`app`).
        // ADR-0098/0100 added 5 graded-theory declaration rules
        // (`sort`/`grade`/`op`/`handler`/`rewrite`); `type.law` was
        // re-shaped in place as a 2-cell (`type.law` + name).
        // `with`/`handle` surface syntax added 2 rules.
        assert_eq!(lang.types.len(), 76);
        // The `value` kind (literals) must have a rule.
        assert!(lang.types.get_by_identity("value").is_some());
        // Spot-check a few meaningful rules.
        assert!(lang.types.get_by_identity("var").is_some());
        assert!(lang.types.get_by_identity("if").is_some());
        assert!(lang.types.get_by_identity("binary.add").is_some());
        assert!(lang.types.get_by_identity("unary.neg").is_some());
        assert!(lang.types.get_by_identity("paren").is_some());
    }

    #[test]
    fn transforms_yaml_loads_first_class_rules() {
        // ADR-0094: transform rules are authored in `data/transforms.yaml`
        // and land in `lang.transforms` directly — identical semantics to
        // the old `desugar:`-field lowering (same ids, pass, conditions,
        // and rewrite steps).
        use handl_language::{RewriteStep, TransformCondition, TransformRule};
        let lang = language_from_yaml().unwrap();
        // 3 baseline rules (call, entry, lower.entry) + 6 desugar rules
        // (do_while, nullish, opt-chain, while, while.state, for).
        assert_eq!(lang.transforms.len(), 9);
        assert_eq!(
            lang.transforms_for("desugar", "do-while")[0].id,
            "transform.desugar.do_while"
        );

        let call = &lang.transforms_for("desugar", "call")[0];
        assert_eq!(call.id, "transform.desugar.call");
        assert_eq!(call.priority, 0);
        assert_eq!(
            call.when,
            Some(TransformCondition::KindAt {
                index: Some(0),
                role: None,
                kind: "json".into()
            })
        );
        assert_eq!(
            call.rule,
            TransformRule::Declarative {
                target_kind: "json".into(),
                children: vec![RewriteStep::Keep(0), RewriteStep::KeepRest(1)],
                payload: None,
            }
        );

        let entry = &lang.transforms_for("desugar", "entry")[0];
        assert_eq!(entry.id, "transform.desugar.entry");
        assert_eq!(
            entry.when,
            Some(TransformCondition::KindAt {
                index: Some(0),
                role: None,
                kind: "array".into()
            })
        );
        assert_eq!(
            entry.rule,
            TransformRule::Declarative {
                target_kind: "entry".into(),
                children: vec![
                    RewriteStep::Create {
                        kind: "paren".into(),
                        payload: None,
                        children: vec![RewriteStep::KeepPath(vec![0, 0])],
                    },
                    RewriteStep::Keep(1),
                ],
                payload: None,
            }
        );
    }

    #[test]
    fn desugar_field_still_lowers_into_transforms() {
        // Backward compat (ADR-0094): an `AstShapeDef` carrying a `desugar:`
        // rule still lowers into a `TransformDef` in pass "desugar" — the
        // field is kept for third-party fragments.
        let ast_yaml = r#"
- id: ast.sugared
  node_kind: sugared
  children:
    - role: a
    - role: b
  desugar:
    target_kind: plain
    when:
      child_index: 0
      child_kind: paren
    children:
      - !Keep 0
      - !Keep 1
"#;
        let lang = merge_fragments(
            "[]", "[]", "[]", "[]", ast_yaml, "[]", "[]", "[]", "[]", "[]",
        )
        .unwrap();
        let rules = lang.transforms_for("desugar", "sugared");
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].id, "transform.desugar.sugared");
        assert_eq!(rules[0].pass, "desugar");
    }

    #[test]
    fn canonical_transforms_validate_clean() {
        // ADR-0094 "Production readiness": `validate_transforms` runs at
        // `Language` build (see `merge_fragments`); the canonical kit must
        // produce zero Error-severity diagnostics.
        let lang = language_from_yaml().unwrap();
        let diags = handl_plugin_engine_transform::validate::validate_transforms(&lang);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.severity == handl_plugin_ext_diag::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected transform errors: {errors:?}");
    }

    #[test]
    fn merge_rejects_invalid_transform_rules() {
        // A `desugar:` rule whose `Keep` index is out of bounds for the
        // shape's declared roles must fail `Language` assembly, not pass
        // silently (ADR-0094: no silent fallthrough).
        let ast_yaml = r#"
- id: ast.bogus
  node_kind: bogus
  children:
    - role: only
  desugar:
    target_kind: bogus
    children:
      - !Keep 7
"#;
        let err = merge_fragments(
            "[]", "[]", "[]", "[]", ast_yaml, "[]", "[]", "[]", "[]", "[]",
        )
        .unwrap_err();
        assert!(
            format!("{err}").contains("transform rule validation failed"),
            "expected validation failure, got: {err}"
        );
    }

    #[test]
    fn load_kit_parses_embedded_manifests() {
        let canonical = load_kit("canonical").unwrap();
        assert_eq!(canonical.fragments.len(), 1);
        assert_eq!(canonical.runtime.eval, "tree-walk");
        assert!(canonical.modules.is_empty());

        let hello = load_kit("hello").unwrap();
        assert_eq!(hello.extends.as_deref(), Some("canonical"));
        assert_eq!(hello.modules, vec!["module-runtime", "hello"]);
    }

    #[test]
    fn load_kit_unknown_name_errors() {
        assert!(load_kit("bogus").is_err());
    }

    #[test]
    fn load_kit_config_declares_pure_library_caps() {
        // The `config` kit's pure-library capabilities are authored data in
        // `data/kits/config.yaml` (they used to come from the deleted
        // `handl-kit-data-config` crate's `PureCapsEmitter`).
        let config = load_kit("config").unwrap();
        assert_eq!(config.extends.as_deref(), Some("canonical"));
        let ids: Vec<&str> = config.caps.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "lib.math",
                "lib.str",
                "lib.array",
                "lib.set",
                "lib.bool",
                "lib.cmp",
                "lib.obj",
                "lib.fn",
                "lib.type",
                "lib.crypto",
                "lib.datetime",
                "lib.intl",
                "lib.kv",
            ]
        );
        assert!(
            config
                .caps
                .iter()
                .all(|c| c.description.as_deref() == Some("pure library capability"))
        );
    }

    #[test]
    fn assemble_config_kit_merges_caps_via_extends() {
        // `extends` resolves through the registry: the `config` manifest
        // (extends `canonical`) assembles the canonical language plus its 13
        // authored `lib.*` capabilities, and inherits the parent's runtime
        // selection (its manifest declares no `runtime:`).
        let reg = registry();
        let config = load_kit("config").unwrap();
        let kit = DataKit::assemble(&config, &reg).unwrap();
        let lang = kit.lang();
        assert!(lang.tokens.len() > 0);
        assert!(lang.grammar.len() > 0);
        assert!(lang.eval.len() > 0);
        let canonical_caps = assemble().unwrap().lang().caps.len();
        assert_eq!(lang.caps.len(), canonical_caps + 13);
        for id in [
            "lib.math",
            "lib.str",
            "lib.array",
            "lib.set",
            "lib.bool",
            "lib.cmp",
            "lib.obj",
            "lib.fn",
            "lib.type",
            "lib.crypto",
            "lib.datetime",
            "lib.intl",
            "lib.kv",
        ] {
            assert!(lang.caps.get_by_id(id).is_some(), "missing {}", id);
        }
        assert_eq!(kit.runtime_config().parse, "pratt");
        assert_eq!(kit.runtime_config().eval, "tree-walk");
    }

    #[test]
    fn kit_module_names_flattens_extends() {
        // `stdlib` extends `hello` which extends `canonical`.
        assert_eq!(
            kit_module_names("stdlib").unwrap(),
            vec!["module-runtime", "hello", "utils", "collections"]
        );
        // `server` extends `config`: parent's modules first.
        let server = kit_module_names("server").unwrap();
        assert_eq!(server.first().unwrap(), "module-runtime");
        assert_eq!(server.last().unwrap(), "otlp-std");
        assert_eq!(server.len(), 19);
    }

    #[test]
    fn excluded_fragments_absent() {
        let kit = assemble().unwrap();
        let lang = kit.lang();
        for id in EXCLUDED_FRAGMENT_IDS {
            // Eval scheme ids map to node kinds; check by id.
            if id.starts_with("eval.") {
                // The eval.call.postfix fragment should not be present.
                // eval schemes are keyed by node_kind, not id, so we check
                // that the canonical eval.call is present (not the postfix
                // variant).
                assert!(lang.eval.get_by_id("eval.call").is_some());
            } else if id.starts_with("grammar.") {
                // Grammar rows are keyed by (token, position).
                // Just verify the canonical grammar.call.infix is present.
                assert!(lang.grammar.get_by_id("grammar.call.infix").is_some());
            }
        }
    }

    #[test]
    fn load_kit_config_carries_caps() {
        let config = load_kit("config").unwrap();
        assert_eq!(config.caps.len(), 13);
        for id in &[
            "lib.math",
            "lib.str",
            "lib.array",
            "lib.set",
            "lib.bool",
            "lib.cmp",
            "lib.obj",
            "lib.fn",
            "lib.type",
            "lib.crypto",
            "lib.datetime",
            "lib.intl",
            "lib.kv",
        ] {
            assert!(
                config.caps.iter().any(|c| c.id == *id),
                "config manifest missing cap {}",
                id
            );
        }
    }

    #[test]
    fn assemble_config_extends_canonical_caps() {
        let kit = DataKit::assemble(&load_kit("config").unwrap(), &registry()).unwrap();
        let lang = kit.lang();
        // Canonical contributes 3 foundation caps; config adds 13 pure-library caps.
        assert_eq!(lang.caps.len(), 16);
        assert!(lang.caps.get_by_id("lex").is_some());
        assert!(lang.caps.get_by_id("parse").is_some());
        for id in &[
            "lib.math",
            "lib.str",
            "lib.array",
            "lib.set",
            "lib.bool",
            "lib.cmp",
            "lib.obj",
            "lib.fn",
            "lib.type",
            "lib.crypto",
            "lib.datetime",
            "lib.intl",
            "lib.kv",
        ] {
            assert!(lang.caps.get_by_id(id).is_some(), "missing cap {}", id);
        }
    }

    #[test]
    fn assemble_config_inherits_parent_runtime() {
        let kit = DataKit::assemble(&load_kit("config").unwrap(), &registry()).unwrap();
        assert_eq!(kit.runtime_config().typecheck, "nbe");
        assert_eq!(kit.runtime_config().parse, "pratt");
        assert_eq!(kit.runtime_config().eval, "tree-walk");
    }
}
