//! Loads the arrow DSL YAML language definition into a [`Language`] struct.
//!
//! The arrow DSL (ADR-0069) is defined as data files in `data/`:
//! `tokens.yaml`, `lexer_rules.yaml`, `grammar.yaml`, `eval.yaml`, `ast.yaml`.
//! The key semantic difference from the main handl language: `>>>` means
//! `Seq` (arrow composition), not `App` (pipe).
//!
//! This module embeds the YAML files at compile time via `include_str!` and
//! deserializes them into a [`Language`] using the same fragment-merge
//! approach as `handl-kit-data-canonical`.

use std::sync::OnceLock;

use handl_language::{
    AstShapeDef, EvalSchemeDef, GrammarRow, Language, LexerRuleDef, TokenKindDef,
};

// Embed the arrow DSL YAML data files at compile time.
const TOKENS_YAML: &str = include_str!("../data/tokens.yaml");
const LEXER_RULES_YAML: &str = include_str!("../data/lexer_rules.yaml");
const GRAMMAR_YAML: &str = include_str!("../data/grammar.yaml");
const EVAL_YAML: &str = include_str!("../data/eval.yaml");
const AST_YAML: &str = include_str!("../data/ast.yaml");

/// Build the arrow DSL [`Language`] from the embedded YAML data files.
///
/// Each file contains a YAML list of fragments of one type. The fragments are
/// deserialized and merged into a single [`Language`].
///
/// # Errors
///
/// Returns a `String` error message if any YAML file fails to deserialize or
/// a merge fails.
fn build_arrow_language() -> Result<Language, String> {
    let mut lang = Language::new();

    // Token kinds (union -- idempotent merge).
    let tokens: Vec<TokenKindDef> = serde_yaml::from_str(TOKENS_YAML)
        .map_err(|e| format!("tokens.yaml: {}", e))?;
    for frag in tokens {
        lang.tokens.merge(frag).map_err(|e| format!("tokens merge: {e}"))?;
    }

    // Lexer rules (union -- idempotent merge).
    let lexer_rules: Vec<LexerRuleDef> = serde_yaml::from_str(LEXER_RULES_YAML)
        .map_err(|e| format!("lexer_rules.yaml: {}", e))?;
    for frag in lexer_rules {
        lang.lexer_rules.merge(frag).map_err(|e| format!("lexer_rules merge: {e}"))?;
    }

    // Grammar rows (non-union -- pre-resolved, no collisions).
    let grammar: Vec<GrammarRow> = serde_yaml::from_str(GRAMMAR_YAML)
        .map_err(|e| format!("grammar.yaml: {}", e))?;
    for frag in grammar {
        lang.grammar.merge(frag).map_err(|e| format!("grammar merge: {e}"))?;
    }

    // Eval schemes (non-union -- pre-resolved, no collisions).
    let eval: Vec<EvalSchemeDef> = serde_yaml::from_str(EVAL_YAML)
        .map_err(|e| format!("eval.yaml: {}", e))?;
    for frag in eval {
        lang.eval.merge(frag).map_err(|e| format!("eval merge: {e}"))?;
    }

    // AST shapes (union -- idempotent merge).
    let ast: Vec<AstShapeDef> = serde_yaml::from_str(AST_YAML)
        .map_err(|e| format!("ast.yaml: {}", e))?;
    for frag in ast {
        lang.ast.merge(frag).map_err(|e| format!("ast merge: {e}"))?;
    }

    Ok(lang)
}

/// Cached arrow DSL [`Language`], loaded once and reused.
static ARROW_LANG: OnceLock<Language> = OnceLock::new();

/// Return the arrow DSL [`Language`], loading it from the embedded YAML data
/// files on first call and returning the cached value thereafter.
///
/// # Panics
///
/// Panics if the YAML data files fail to deserialize (a programming error --
/// the files are embedded at compile time and should always be valid).
pub fn arrow_language() -> &'static Language {
    ARROW_LANG.get_or_init(|| {
        build_arrow_language().expect("arrow DSL YAML data files must deserialize")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrow_language_loads_successfully() {
        let lang = arrow_language();
        // Should have token kinds for the arrow DSL.
        assert!(lang.tokens.get_by_identity("token.arrow.flow").is_some());
        assert!(lang.tokens.get_by_identity("token.arrow.arr").is_some());
        // Should have grammar rows.
        assert!(lang.grammar.get_by_identity("token.arrow.flow@infix").is_some());
        assert!(lang.grammar.get_by_identity("token.arrow.arr@prefix").is_some());
        // Should have eval schemes.
        assert!(lang.eval.get_by_identity("flow").is_some());
        assert!(lang.eval.get_by_identity("arr_call").is_some());
        assert!(lang.eval.get_by_identity("arrow_id").is_some());
        // Should have lexer rules.
        assert!(lang.lexer_rules.get_by_identity("lex.arrow.flow").is_some());
    }

    #[test]
    fn flow_eval_is_seq_not_app() {
        let lang = arrow_language();
        let eval = lang.eval.get_by_identity("flow").expect("flow eval scheme");
        assert!(
            matches!(eval.scheme, handl_language::Scheme::Seq { .. }),
            ">>> (flow) must lower to Seq in the arrow DSL, got {:?}",
            eval.scheme
        );
    }

    #[test]
    fn choice_op_eval_is_choice() {
        let lang = arrow_language();
        let eval = lang.eval.get_by_identity("choice_op").expect("choice_op eval scheme");
        assert!(
            matches!(eval.scheme, handl_language::Scheme::Choice { .. }),
            "||| (choice_op) must lower to Choice, got {:?}",
            eval.scheme
        );
    }
}
