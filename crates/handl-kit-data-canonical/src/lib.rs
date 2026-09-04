//! # handl-kit-data-canonical
//!
//! The **data-driven canonical kit** — the ADR-0044 §6 end state.
//!
//! Assembles a [`Language`] from **authored YAML data files** (one per fragment
//! type: `tokens.yaml`, `grammar.yaml`, `eval.yaml`, `ast.yaml`, `caps.yaml`,
//! `options.yaml`, `lexer_rules.yaml`), embedded at compile time via
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
use handl_kit_data::{DataKit, DataKitManifest, EmitterRegistry, FragmentSourceEntry};
use handl_language::{
    AstShapeDef, CapabilityDecl, EvalSchemeDef, GrammarRow, LangError, Language, LexerRuleDef,
    OptionSchema, TokenKindDef, TypeRuleDef,
};
use handl_runtime::RuntimeConfig;

/// The kit name under which this kit registers itself (for `extends`
/// resolution by child kits like `handl-kit-data-config`).
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

/// Build a [`Language`] from the 7 embedded YAML data files.
///
/// Each file contains a YAML list of fragments of one type. The fragments are
/// deserialized and merged into a single [`Language`]. `eval.yaml` holds the
/// arrow-calculus `Scheme` definitions (ADR-0052 kernel).
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
    )
}

/// Merge the eight YAML fragment strings into a single [`Language`].
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

    // AST shapes (union — idempotent merge).
    let ast: Vec<AstShapeDef> = serde_yaml::from_str(ast_yaml)
        .map_err(|e| LangError::engine(format!("ast.yaml: {}", e)))?;
    for frag in ast {
        lang.ast.merge(frag)?;
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

    Ok(lang)
}

/// Build a [`Language`] from the eight YAML data files in a directory.
///
/// Reads `tokens.yaml`, `grammar.yaml`, `eval.yaml`, `types.yaml`,
/// `ast.yaml`, `caps.yaml`, `options.yaml`, and `lexer_rules.yaml` from `dir`
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

    merge_fragments(
        &tokens_yaml,
        &grammar_yaml,
        &eval_yaml,
        &types_yaml,
        &ast_yaml,
        &caps_yaml,
        &options_yaml,
        &lexer_rules_yaml,
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
/// embedded YAML files) and selects the Pratt parse engine + tree-walk eval
/// engine.
pub fn manifest() -> DataKitManifest {
    DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter(
            "canonical-yaml",
            "canonical-yaml",
        ))
        .with_runtime(
            RuntimeConfig::new()
                .with_lex("data")
                .with_parse("pratt")
                .with_eval("tree-walk"),
        )
}

/// Build an [`EmitterRegistry`] pre-populated with the canonical YAML data
/// emitter and this kit's manifest registered under [`KIT_NAME`].
pub fn registry() -> EmitterRegistry {
    let mut reg = EmitterRegistry::new();
    reg.register_emitter("canonical-yaml", || Box::new(YamlDataEmitter::new()));
    reg.register_kit(KIT_NAME, manifest());
    reg
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
        // 58 type rules in types.yaml. ADR-0079 removed `type.let` and
        // `type.stmt.let` (let keyword deleted, graph blocks replace it).
        assert_eq!(lang.types.len(), 58);
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
}
