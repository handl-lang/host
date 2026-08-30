//! # handl-kit-data-canonical
//!
//! The **data-driven canonical kit** — the ADR-0044 Phase 5 data-driven
//! equivalent of the pre-migration `kit-canonical` Rust monolith, now covering
//! the **full data-driven language** (all Phase 4 emitters).
//!
//! Assembles a [`Language`] from the complete set of data-driven emitters
//! (the foundation `tokens` emitter plus every grammar/eval emitter) and
//! selects `RuntimeConfig { parse: "pratt", eval: "tree-walk" }`. This is the
//! realization of ADR-0044 §1: "selection and merge are the composition; the
//! registry is replaced by a `Language` built from merged fragments."
//!
//! The existing `kit-canonical` crate is **not** modified or deleted — it
//! remains as the pre-migration reference. This crate builds the new
//! data-driven path that runs alongside it.
//!
//! ## Collision handling
//!
//! The 39 Phase 4 emitters include several **alternative** implementations of
//! the same construct (e.g. `handl-emitter-function` vs the split
//! `handl-emitter-arrow` + `handl-emitter-function-literal`; `apply` vs
//! `paren` for the `(` prefix slot; `eval-core` vs `identifier`/`apply` for
//! the `var`/`apply` eval schemes). Because `GrammarRow` and `EvalSchemeDef`
//! are **non-union** (behavioral) fragment types, two fragments with the same
//! identity key conflict unless one declares `replaces`. The kit resolves
//! these as follows:
//!
//! - **Token kinds & AST shapes** are union (idempotent) fragment types
//!   (`TokenKindDef::UNION = true`, `AstShapeDef::UNION = true`), so the many
//!   emitters that re-declare `token.lparen`, `ast.var`, etc. merge
//!   idempotently (first declaration wins, duplicates silently dropped).
//! - **Eval scheme collisions on the same node kind** are resolved by
//!   excluding the non-canonical duplicate fragment ids via
//!   [`DataKitManifest::with_exclude`], and by trimming `eval-core` to its
//!   unique `resolve` primitive (see `handl-emitter-eval-core` docs).
//! - **Grammar row collisions on the same `(token, position)`** are resolved
//!   by excluding the non-canonical duplicate grammar row id.
//! - The pure alternative emitters `handl-emitter-arrow` and
//!   `handl-emitter-function-literal` (the split grammar/eval halves of the
//!   function construct) share the `grammar.arrow.infix` / `eval.function`
//!   fragment ids with the canonical `handl-emitter-function`. They are
//!   registered in the [`EmitterRegistry`] and listed in [`EMITTER_NAMES`]
//!   for child kits, but are **not** merged into this kit's manifest (their
//!   same-id duplicates cannot be excluded without also dropping the
//!   canonical `function` fragments).
//!
//! ## Usage
//!
//! ```no_run
//! use handl_kit_data::EmitterRegistry;
//! let kit = handl_kit_data_canonical::assemble().unwrap();
//! let lang = kit.lang();
//! ```

#![warn(missing_docs)]

use handl_emitter_apply::ApplyEmitter;
use handl_emitter_arith::ArithEmitter;
use handl_emitter_array::ArrayEmitter;
use handl_emitter_arrow::ArrowEmitter;
use handl_emitter_assign::AssignEmitter;
use handl_emitter_branch::BranchEmitter;
use handl_emitter_call::CallEmitter;
use handl_emitter_cmp::CmpEmitter;
use handl_emitter_eval_core::EvalCoreEmitter;
use handl_emitter_eval_hello::EvalHelloEmitter;
use handl_emitter_eval_json::EvalJsonEmitter;
use handl_emitter_fanout::FanoutEmitter;
use handl_emitter_flow::FlowEmitter;
use handl_emitter_function::FunctionEmitter;
use handl_emitter_function_literal::FunctionLiteralEmitter;
use handl_emitter_handle::HandleEmitter;
use handl_emitter_identifier::IdentifierEmitter;
use handl_emitter_let::LetEmitter;
use handl_emitter_match::MatchEmitter;
use handl_emitter_not::NotEmitter;
use handl_emitter_object::ObjectEmitter;
use handl_emitter_paren::ParenEmitter;
use handl_emitter_pipe::PipeEmitter;
use handl_emitter_postfix_call::PostfixCallEmitter;
use handl_emitter_postfix_field::PostfixFieldEmitter;
use handl_emitter_postfix_index::PostfixIndexEmitter;
use handl_emitter_postfix_mood::PostfixMoodEmitter;
use handl_emitter_stmt_const::StmtConstEmitter;
use handl_emitter_stmt_export::StmtExportEmitter;
use handl_emitter_stmt_import::StmtImportEmitter;
use handl_emitter_stmt_let::StmtLetEmitter;
use handl_emitter_stmt_module::StmtModuleEmitter;
use handl_emitter_template::TemplateEmitter;
use handl_emitter_ternary::TernaryEmitter;
use handl_emitter_textmate::TextmateEmitter;
use handl_emitter_tokens::TokensEmitter;
use handl_emitter_type_annot::TypeAnnotEmitter;
use handl_emitter_typeof::TypeofEmitter;
use handl_emitter_value::ValueEmitter;
use handl_kit_data::{
    DataKit, DataKitManifest, EmitterRegistry, FragmentSourceEntry,
};
use handl_language::LangError;
use handl_runtime::RuntimeConfig;

/// The kit name under which this kit registers itself (for `extends`
/// resolution by child kits like `handl-kit-data-config`).
pub const KIT_NAME: &str = "kit-data-canonical";

/// All Phase 4 emitter names contributed by this kit's registry, in merge
/// order (the foundation `tokens` emitter first, then the rest).
///
/// Every emitter is registered in [`registry()`] so child kits that `extends`
/// this kit can reference them. The kit's [`manifest()`] merges all of these
/// except the pure alternative emitters `arrow` and `function-literal` (see
/// the crate-level docs on collision handling).
pub const EMITTER_NAMES: &[&str] = &[
    "tokens",
    "identifier",
    "value",
    "arith",
    "let",
    "call",
    "branch",
    "function",
    "cmp",
    "assign",
    "pipe",
    "flow",
    "fanout",
    "ternary",
    "not",
    "paren",
    "typeof",
    "array",
    "object",
    "match",
    "handle",
    "postfix-call",
    "postfix-field",
    "postfix-index",
    "postfix-mood",
    "apply",
    "arrow",
    "function-literal",
    "template",
    "type-annot",
    "textmate",
    "stmt-let",
    "stmt-const",
    "stmt-import",
    "stmt-export",
    "stmt-module",
    "eval-core",
    "eval-hello",
    "eval-json",
];

/// Fragment ids excluded from the canonical kit merge.
///
/// These are non-canonical duplicate fragments (behavioral, non-union types)
/// that share an identity key with a canonical fragment from another emitter.
/// Excluding them by id keeps the canonical fragment and drops the duplicate:
///
/// - `eval.call.postfix` — from `postfix-call`; `call`'s `eval.call` is the
///   canonical eval scheme for the `call` node kind.
/// - `grammar.call.postfix` — from `postfix-call`; `call`'s
///   `grammar.call.infix` is the canonical infix rule for the `(` token
///   (`postfix-call` encodes its postfix `(` as an infix row with the same
///   `token.lparen@infix` identity key). (`postfix-call`'s token kinds are
///   kept — they are union/idempotent.)
/// - `grammar.apply.prefix` — from `apply`; `paren`'s `grammar.paren.prefix`
///   is the canonical prefix rule for the `(` token (grouping). (`apply`'s
///   `eval.apply` scheme and `token.lparen` declaration are kept — only the
///   duplicate grammar row is dropped.)
pub const EXCLUDED_FRAGMENT_IDS: &[&str] = &[
    "eval.call.postfix",
    "grammar.call.postfix",
    "grammar.apply.prefix",
];

/// Build the [`DataKitManifest`] for the data-driven canonical kit.
///
/// The manifest lists the full-language emitters as fragment sources (in
/// merge order — the foundation `tokens` emitter first, then the rest) and
/// selects the Pratt parse engine + tree-walk eval engine. The two pure
/// alternative emitters (`arrow`, `function-literal`) are omitted from the
/// manifest because they share same-id behavioral fragments with the
/// canonical `function` emitter (see the crate-level docs).
pub fn manifest() -> DataKitManifest {
    DataKitManifest::new()
        // Foundation: all token kinds + AST shapes + lex/parse capabilities.
        .with_fragment(FragmentSourceEntry::emitter("tokens", "tokens"))
        // Original Phase 4 subset.
        .with_fragment(FragmentSourceEntry::emitter("identifier", "identifier"))
        .with_fragment(FragmentSourceEntry::emitter("value", "value"))
        .with_fragment(FragmentSourceEntry::emitter("arith", "arith"))
        .with_fragment(FragmentSourceEntry::emitter("let", "let"))
        .with_fragment(FragmentSourceEntry::emitter("call", "call"))
        .with_fragment(FragmentSourceEntry::emitter("branch", "branch"))
        .with_fragment(FragmentSourceEntry::emitter("function", "function"))
        // Full-language grammar/eval emitters.
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
        .with_fragment(FragmentSourceEntry::emitter(
            "postfix-call",
            "postfix-call",
        ))
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
        // Drop non-canonical duplicate behavioral fragments (see
        // EXCLUDED_FRAGMENT_IDS docs).
        .with_exclude(EXCLUDED_FRAGMENT_IDS.iter().map(|s| s.to_string()).collect())
        .with_runtime(
            RuntimeConfig::new()
                .with_lex("data")
                .with_parse("pratt")
                .with_eval("tree-walk"),
        )
}

/// Build an [`EmitterRegistry`] pre-populated with all Phase 4 emitters and
/// this kit's manifest registered under [`KIT_NAME`].
///
/// This is the bridge between the manifest's emitter names and the Rust
/// emitter constructors. Child kits that `extends` this kit reuse this
/// registry (adding their own emitters/manifests on top). All 39 Phase 4
/// emitters are registered here, including the alternative emitters (`arrow`,
/// `function-literal`) that are not merged into this kit's manifest but are
/// available for child kits that prefer the split function-construct design.
pub fn registry() -> EmitterRegistry {
    let mut reg = EmitterRegistry::new();
    reg.register_emitter("tokens", || Box::new(TokensEmitter::new()));
    reg.register_emitter("identifier", || {
        Box::new(IdentifierEmitter::new())
    });
    reg.register_emitter("value", || Box::new(ValueEmitter::new()));
    reg.register_emitter("arith", || Box::new(ArithEmitter::new()));
    reg.register_emitter("let", || Box::new(LetEmitter::new()));
    reg.register_emitter("call", || Box::new(CallEmitter::new()));
    reg.register_emitter("branch", || Box::new(BranchEmitter::new()));
    reg.register_emitter("function", || Box::new(FunctionEmitter::new()));
    reg.register_emitter("cmp", || Box::new(CmpEmitter::new()));
    reg.register_emitter("assign", || Box::new(AssignEmitter::new()));
    reg.register_emitter("pipe", || Box::new(PipeEmitter::new()));
    reg.register_emitter("flow", || Box::new(FlowEmitter::new()));
    reg.register_emitter("fanout", || Box::new(FanoutEmitter::new()));
    reg.register_emitter("ternary", || Box::new(TernaryEmitter::new()));
    reg.register_emitter("not", || Box::new(NotEmitter::new()));
    reg.register_emitter("paren", || Box::new(ParenEmitter::new()));
    reg.register_emitter("typeof", || Box::new(TypeofEmitter::new()));
    reg.register_emitter("array", || Box::new(ArrayEmitter::new()));
    reg.register_emitter("object", || Box::new(ObjectEmitter::new()));
    reg.register_emitter("match", || Box::new(MatchEmitter::new()));
    reg.register_emitter("handle", || Box::new(HandleEmitter::new()));
    reg.register_emitter("postfix-call", || {
        Box::new(PostfixCallEmitter::new())
    });
    reg.register_emitter("postfix-field", || {
        Box::new(PostfixFieldEmitter::new())
    });
    reg.register_emitter("postfix-index", || {
        Box::new(PostfixIndexEmitter::new())
    });
    reg.register_emitter("postfix-mood", || {
        Box::new(PostfixMoodEmitter::new())
    });
    reg.register_emitter("apply", || Box::new(ApplyEmitter::new()));
    reg.register_emitter("arrow", || Box::new(ArrowEmitter::new()));
    reg.register_emitter("function-literal", || {
        Box::new(FunctionLiteralEmitter::new())
    });
    reg.register_emitter("template", || Box::new(TemplateEmitter::new()));
    reg.register_emitter("type-annot", || Box::new(TypeAnnotEmitter::new()));
    reg.register_emitter("textmate", || Box::new(TextmateEmitter::new()));
    reg.register_emitter("stmt-let", || Box::new(StmtLetEmitter::new()));
    reg.register_emitter("stmt-const", || Box::new(StmtConstEmitter::new()));
    reg.register_emitter("stmt-import", || Box::new(StmtImportEmitter::new()));
    reg.register_emitter("stmt-export", || Box::new(StmtExportEmitter::new()));
    reg.register_emitter("stmt-module", || Box::new(StmtModuleEmitter::new()));
    reg.register_emitter("eval-core", || Box::new(EvalCoreEmitter::new()));
    reg.register_emitter("eval-hello", || Box::new(EvalHelloEmitter::new()));
    reg.register_emitter("eval-json", || Box::new(EvalJsonEmitter::new()));
    reg.register_kit(KIT_NAME, manifest());
    reg
}

/// Assemble the data-driven canonical kit: register all Phase 4 emitters,
/// build the manifest, and assemble a [`DataKit`].
///
/// # Errors
///
/// Returns [`LangError`] if any fragment merge fails (e.g. an unexpected
/// collision — should not happen for the canonical subset).
pub fn assemble() -> Result<DataKit, LangError> {
    let reg = registry();
    DataKit::assemble(&manifest(), &reg)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Number of fragment sources listed in the canonical manifest.
    ///
    /// All 39 Phase 4 emitters except the two pure alternative emitters
    /// (`arrow`, `function-literal`) that share same-id behavioral fragments
    /// with the canonical `function` emitter.
    const MANIFEST_FRAGMENT_SOURCES: usize = 37;

    #[test]
    fn manifest_has_full_language_fragments() {
        let m = manifest();
        assert_eq!(m.fragments.len(), MANIFEST_FRAGMENT_SOURCES);
        assert_eq!(m.extends, None);
        assert_eq!(m.runtime.lex.as_deref(), Some("data"));
        assert_eq!(m.runtime.parse.as_deref(), Some("pratt"));
        assert_eq!(m.runtime.eval.as_deref(), Some("tree-walk"));
        assert_eq!(m.exclude, EXCLUDED_FRAGMENT_IDS.to_vec());
        // The foundation `tokens` emitter is first (merge order).
        assert_eq!(m.fragments.first().unwrap().name, "tokens");
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
        assert!(lang.ast.len() > 0);
        assert!(lang.caps.len() > 0);
    }

    #[test]
    fn registry_resolves_all_emitters() {
        let reg = registry();
        for name in EMITTER_NAMES {
            assert!(
                reg.resolve_emitter(name).is_some(),
                "emitter {} should resolve",
                name
            );
        }
        assert_eq!(EMITTER_NAMES.len(), 39);
    }

    #[test]
    fn registry_resolves_canonical_kit_manifest() {
        let reg = registry();
        let m = reg.resolve_kit(KIT_NAME).unwrap();
        assert_eq!(m.fragments.len(), MANIFEST_FRAGMENT_SOURCES);
    }

    #[test]
    fn assemble_into_runtime_binds_engines() {
        let kit = assemble().unwrap();
        let rt = kit.into_runtime(None).unwrap();
        assert_eq!(
            rt.lang().tokens.len(),
            assemble().unwrap().lang().tokens.len()
        );
        // The lex engine is bound ("data"): lexing a simple expression with the
        // canonical lexer rules produces a token stream rather than
        // NotImplemented.
        let toks = rt
            .lex(&handl_language::Source::new("1 + 2"))
            .unwrap();
        assert_eq!(toks.as_slice().len(), 3);
        assert_eq!(toks.as_slice()[0].kind, "token.number");
        assert_eq!(toks.as_slice()[1].kind, "token.plus");
        assert_eq!(toks.as_slice()[2].kind, "token.number");
    }
}
