//! Loads the arrow DSL language definition.
//!
//! Since ADR-0079, the arrow dialect uses the canonical handl language
//! directly. Arrow-specific keywords (`arr`, `comp`, `first`, `choice`,
//! `loop`, `app`, `id`) are handled as regular identifiers by the parser,
//! which dispatches on callee names in `materialize_node`.
//!
//! # Precedence override
//!
//! The canonical grammar binds `>>>` (flow, bp 25/26) tighter than `|||`
//! (choice, bp 20/21). The arrow DSL requires the **opposite**: `|||` binds
//! tighter than `>>>` so that `a >>> b ||| c` parses as `a >>> (b ||| c)`
//! (= `Seq(a, Choice(b, c))`). We override the two grammar rows after
//! loading the canonical language to swap their binding powers.

use std::sync::OnceLock;

use handl_language::{
    AstShapeRef, DelimitedShape, GrammarPosition, GrammarRow, Language, PairShape,
};

/// Cached arrow DSL [`Language`], loaded once and reused.
static ARROW_LANG: OnceLock<Language> = OnceLock::new();

/// Return the arrow DSL [`Language`], which is the canonical handl language
/// (ADR-0079 unification) with arrow-specific precedence overrides for
/// `>>>` (flow) and `|||` (choice).
///
/// # Panics
///
/// Panics if the canonical YAML data files fail to deserialize or if the
/// grammar row overrides fail to merge.
pub fn arrow_language() -> &'static Language {
    ARROW_LANG.get_or_init(|| {
        let mut lang = handl_kit_data_canonical::language_from_yaml()
            .expect("canonical YAML data files must deserialize");

        // Override `>>>` (flow) binding power: lower it so `|||` binds
        // tighter. Canonical is (25, 26); arrow needs (20, 21).
        lang.grammar
            .merge(
                GrammarRow::new(
                    "grammar.flow.infix.arrow",
                    "token.flow",
                    GrammarPosition::Infix,
                    AstShapeRef::new("flow", 2),
                )
                .with_binding_power(20, 21)
                .with_replaces(vec!["grammar.flow.infix".to_string()]),
            )
            .expect("override flow grammar row");

        // Override `|||` (choice) binding power: raise it so it binds
        // tighter than `>>>`. Canonical is (20, 21); arrow needs (25, 26).
        lang.grammar
            .merge(
                GrammarRow::new(
                    "grammar.choice.infix.arrow",
                    "token.choice",
                    GrammarPosition::Infix,
                    AstShapeRef::new("choice", 2),
                )
                .with_binding_power(25, 26)
                .with_replaces(vec!["grammar.choice.infix".to_string()]),
            )
            .expect("override choice grammar row");

        // Override `{ ... }` to produce a `block` node (not an `object`).
        // The arrow DSL uses `{ ... }` for both state blocks (`key: value`
        // pairs) and code blocks (`name = expr` assignments, `if/else`).
        // The canonical grammar parses bare `{ ... }` as an `object` (comma-
        // separated `key: value` entries), which fails on `name = expr`
        // assignments. We replace it with a block that uses newline
        // separation (commas tolerated) and `:` as the pair separator,
        // matching the old arrow-specific grammar's `arrow_block` rule.
        lang.grammar
            .merge(
                GrammarRow::new(
                    "grammar.block.prefix.arrow",
                    "token.lbrace",
                    GrammarPosition::Prefix,
                    AstShapeRef::new("block", 1)
                        .with_delimited(DelimitedShape {
                            open: "token.lbrace".into(),
                            close: "token.rbrace".into(),
                            separator: None,
                            container_kind: "block".into(),
                            pair: Some(PairShape {
                                separator: "token.colon".into(),
                                node_kind: "entry".into(),
                            }),
                            open_is_operator_token: true,
                            flatten: true,
                            rest_token: None,
                        }),
                )
                .with_binding_power(0, 0)
                .with_replaces(vec!["grammar.object.prefix".to_string()]),
            )
            .expect("override block grammar row");

        lang
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrow_language_loads_successfully() {
        let lang = arrow_language();
        // Should have canonical token kinds.
        assert!(lang.tokens.get_by_identity("token.flow").is_some());
        assert!(lang.tokens.get_by_identity("token.choice").is_some());
        // Should have canonical grammar rows.
        assert!(lang.grammar.get_by_identity("token.flow@infix").is_some());
        // Should have canonical eval schemes.
        assert!(lang.eval.get_by_identity("flow").is_some());
    }

    #[test]
    fn arrow_flow_binds_looser_than_choice() {
        // ADR-0079: in the arrow DSL, `|||` (choice) binds tighter than
        // `>>>` (flow), so `a >>> b ||| c` parses as `a >>> (b ||| c)`.
        // The canonical grammar has the opposite precedence; the arrow
        // language overrides it.
        let lang = arrow_language();
        let flow_row = lang
            .grammar
            .get_by_identity("token.flow@infix")
            .expect("flow grammar row");
        let choice_row = lang
            .grammar
            .get_by_identity("token.choice@infix")
            .expect("choice grammar row");
        let flow_bp = flow_row.binding_power.expect("flow has binding power");
        let choice_bp = choice_row.binding_power.expect("choice has binding power");
        assert!(
            choice_bp.0 > flow_bp.0,
            "choice (bp {:?}) should bind tighter than flow (bp {:?})",
            choice_bp,
            flow_bp
        );
    }

    #[test]
    fn arrow_lbrace_produces_block_not_object() {
        // ADR-0079: the arrow DSL uses `{ ... }` for blocks (with `=`
        // assignments and `if/else`), not just objects. The arrow language
        // overrides the canonical `object` prefix to produce a `block`.
        let lang = arrow_language();
        let row = lang
            .grammar
            .get_by_identity("token.lbrace@prefix")
            .expect("lbrace grammar row");
        assert_eq!(
            row.child_shape.node_kind, "block",
            "lbrace should produce 'block' in arrow DSL, got '{}'",
            row.child_shape.node_kind
        );
    }
}
