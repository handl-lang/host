//! Loads the arrow DSL language definition into a [`Language`] struct.
//!
//! Starting from ADR-0079, the arrow DSL is built on top of the canonical
//! handl language. The canonical language (`handl-kit-data-canonical`)
//! provides the base tokens, lexer rules, grammar, eval schemes, and AST
//! shapes. Arrow-specific fragments (keywords like `arr`/`comp`/`first`,
//! dotted identifiers, `>>>`→`Seq` eval, `loop_rich`/`rec` grammar) are
//! merged on top as an overlay.
//!
//! This approach unifies the two dialects at the engine level while
//! preserving arrow-specific lexer behavior (e.g. `.` in identifiers for
//! `prim.compare` and `state.acc`) that cannot be expressed in the
//! canonical identifier rule without breaking field-access parsing.

use std::sync::OnceLock;

use handl_language::{
    AstShapeDef, EvalSchemeDef, GrammarRow, Language, LexerRuleDef, TokenKindDef,
};

// Embed the arrow-specific overlay YAML data files at compile time.
// These contain ONLY arrow-specific fragments — the base language comes
// from `handl_kit_data_canonical::language_from_yaml()`.
const TOKENS_YAML: &str = include_str!("../data/tokens.yaml");
const LEXER_RULES_YAML: &str = include_str!("../data/lexer_rules.yaml");
const GRAMMAR_YAML: &str = include_str!("../data/grammar.yaml");
const EVAL_YAML: &str = include_str!("../data/eval.yaml");
const AST_YAML: &str = include_str!("../data/ast.yaml");

/// Build the arrow DSL [`Language`] by loading the canonical language as a
/// base and merging arrow-specific fragments on top (ADR-0079).
///
/// The canonical language provides all base tokens, lexer rules, grammar
/// rows, eval schemes, and AST shapes. The arrow overlay adds:
/// - Arrow keywords (`arr`, `comp`, `first`, `choice`, `loop`, `rec`,
///   `app`, `id`) and their grammar rows
/// - Dotted identifiers (`prim.compare`, `state.acc`) via a separate
///   lexer rule
/// - `>>>` → `Scheme::Seq` eval (replaces canonical's `App`)
/// - Arrow-specific node kinds (`arr_call`, `first_call`, `loop_rich`,
///   `arrow_var`, `arrow_match`, etc.)
///
/// # Errors
///
/// Returns a `String` error message if any YAML file fails to deserialize
/// or a merge fails.
fn build_arrow_language() -> Result<Language, String> {
    // Start with the canonical language as base (ADR-0079 unification).
    let mut lang = handl_kit_data_canonical::language_from_yaml()
        .map_err(|e| format!("canonical language: {}", e))?;

    // Merge arrow-specific token kinds (union -- idempotent merge).
    let tokens: Vec<TokenKindDef> = serde_yaml::from_str(TOKENS_YAML)
        .map_err(|e| format!("tokens.yaml: {}", e))?;
    for frag in tokens {
        lang.tokens.merge(frag).map_err(|e| format!("tokens merge: {e}"))?;
    }

    // Merge arrow-specific lexer rules (union -- idempotent merge).
    let lexer_rules: Vec<LexerRuleDef> = serde_yaml::from_str(LEXER_RULES_YAML)
        .map_err(|e| format!("lexer_rules.yaml: {}", e))?;
    for frag in lexer_rules {
        lang.lexer_rules.merge(frag).map_err(|e| format!("lexer_rules merge: {e}"))?;
    }

    // Merge arrow-specific grammar rows (non-union -- no collisions with
    // canonical because arrow rows use `token.arrow.*` token names).
    let grammar: Vec<GrammarRow> = serde_yaml::from_str(GRAMMAR_YAML)
        .map_err(|e| format!("grammar.yaml: {}", e))?;
    for frag in grammar {
        lang.grammar.merge(frag).map_err(|e| format!("grammar merge: {e}"))?;
    }

    // Merge arrow-specific eval schemes (non-union). The `eval.arrow.flow`
    // entry uses `replaces: ["eval.flow"]` to override canonical's `App`
    // with `Seq` for the `>>>` operator.
    let eval: Vec<EvalSchemeDef> = serde_yaml::from_str(EVAL_YAML)
        .map_err(|e| format!("eval.yaml: {}", e))?;
    for frag in eval {
        lang.eval.merge(frag).map_err(|e| format!("eval merge: {e}"))?;
    }

    // Merge arrow-specific AST shapes (union -- idempotent merge).
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
