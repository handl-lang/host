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

use handl_language::ArrSource;
use handl_language::{
    AstShapeRef, ChildSeparator, DelimitedShape, EvalSchemeDef, GrammarPosition, GrammarRow,
    Language, PairShape,
};
use hnl_engine::TermData;

/// Cached arrow DSL [`Language`], loaded once and reused.
/// Derive the arrow DSL [`Language`] from `base`: the canonical handl
/// language (ADR-0079 unification) with arrow-specific precedence overrides
/// for `>>>` (flow) and `|||` (choice), plus the arrow block grammar and
/// arrow-calculus eval schemes.
///
/// The *base* language is injected by the host (ADR-0128 §"The 0.8 seam"):
/// this crate stays in `platform`, so it must not link
/// `handl-kit-data-prelude` — the caller supplies the canonical language
/// (e.g. `handl_kit_data_prelude::language_from_yaml()`). Hosts that call
/// it repeatedly should memoize the result.
///
/// # Panics
///
/// Panics if the grammar row / eval scheme overrides fail to merge (the
/// overrides assume the canonical row identities exist in `base`).
pub fn arrow_language_for(base: &Language) -> Language {
    {
        let mut lang = base.clone();

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

        // Override `>>>[k]` (axis_flow) binding power to match `>>>` in the
        // arrow dialect (20, 21). The canonical row (25, 26) is replaced; the
        // `]` separator still captures the axis expression (ADR-0120 Phase 2).
        lang.grammar
            .merge(
                GrammarRow::new(
                    "grammar.axis_flow.infix.arrow",
                    "token.flow_axis_open",
                    GrammarPosition::Infix,
                    AstShapeRef::new("axis_flow", 3).with_child_separators(vec![
                        ChildSeparator::Required("token.rbracket".to_string()),
                    ]),
                )
                .with_binding_power(20, 21)
                .with_replaces(vec!["grammar.axis_flow.infix".to_string()]),
            )
            .expect("override axis_flow grammar row");

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
                    AstShapeRef::new("block", 1).with_delimited(DelimitedShape {
                        open: "token.lbrace".into(),
                        close: "token.rbrace".into(),
                        separator: None,
                        container_kind: "block".into(),
                        pair: Some(PairShape {
                            separator: "token.colon".into(),
                            node_kind: "entry".into(),
                            record: true,
                        }),
                        open_is_operator_token: true,
                        flatten: true,
                        rest_token: None,
                        newline_separator: false,
                    }),
                )
                .with_binding_power(0, 0)
                .with_replaces(vec!["grammar.object.prefix".to_string()]),
            )
            .expect("override block grammar row");

        // Override the runtime-oriented eval schemes for `>>>` and comparison
        // operators with arrow-calculus `Term` templates. This lets the arrow
        // materializer avoid hardcoded `kind == "flow"` / `kind == "binary.*"`
        // branches: it walks the data-driven eval scheme instead.
        let flow = TermData::seq(
            TermData::op(
                "proj_child".to_string(),
                vec![],
                &ArrSource::Native,
                Some(serde_json::Value::from(0u64)),
            ),
            TermData::op(
                "proj_child".to_string(),
                vec![],
                &ArrSource::Native,
                Some(serde_json::Value::from(1u64)),
            ),
        )
        .to_term()
        .expect("arrow_lang: flow TermData -> Term failed");
        lang.eval
            .merge(
                EvalSchemeDef::new("eval.flow.arrow", "flow", flow)
                    .with_replaces(vec!["eval.flow".to_string()]),
            )
            .expect("override flow eval scheme");

        // `a >>>[k] b` (axis_flow): materialize to `Term::Seq` whose axis
        // slot carries the materialized axis expression (child 1), with the
        // left (0) and right (2) operands as `fst`/`snd` — the same
        // proj_child template substitution the `flow` scheme uses
        // (ADR-0120 Phase 2).
        let axis_flow = TermData::seq_axis(
            TermData::op(
                "proj_child".to_string(),
                vec![],
                &ArrSource::Native,
                Some(serde_json::Value::from(1u64)),
            ),
            TermData::op(
                "proj_child".to_string(),
                vec![],
                &ArrSource::Native,
                Some(serde_json::Value::from(0u64)),
            ),
            TermData::op(
                "proj_child".to_string(),
                vec![],
                &ArrSource::Native,
                Some(serde_json::Value::from(2u64)),
            ),
        )
        .to_term()
        .expect("arrow_lang: axis_flow TermData -> Term failed");
        lang.eval
            .merge(
                EvalSchemeDef::new("eval.axis_flow.arrow", "axis_flow", axis_flow)
                    .with_replaces(vec!["eval.axis_flow".to_string()]),
            )
            .expect("override axis_flow eval scheme");

        let bin_lt = TermData::app(
            TermData::op("lt".to_string(), vec![], &ArrSource::Native, None),
            vec![
                TermData::op(
                    "proj_child".to_string(),
                    vec![],
                    &ArrSource::Native,
                    Some(serde_json::Value::from(0u64)),
                ),
                TermData::op(
                    "proj_child".to_string(),
                    vec![],
                    &ArrSource::Native,
                    Some(serde_json::Value::from(1u64)),
                ),
            ],
        )
        .to_term()
        .expect("arrow_lang: bin_lt TermData -> Term failed");
        lang.eval
            .merge(
                EvalSchemeDef::new("eval.binary.lt.arrow", "binary.lt", bin_lt)
                    .with_replaces(vec!["eval.binary.lt".to_string()]),
            )
            .expect("override binary.lt eval scheme");

        let bin_lte = TermData::app(
            TermData::op("lte".to_string(), vec![], &ArrSource::Native, None),
            vec![
                TermData::op(
                    "proj_child".to_string(),
                    vec![],
                    &ArrSource::Native,
                    Some(serde_json::Value::from(0u64)),
                ),
                TermData::op(
                    "proj_child".to_string(),
                    vec![],
                    &ArrSource::Native,
                    Some(serde_json::Value::from(1u64)),
                ),
            ],
        )
        .to_term()
        .expect("arrow_lang: bin_lte TermData -> Term failed");
        lang.eval
            .merge(
                EvalSchemeDef::new("eval.binary.lte.arrow", "binary.lte", bin_lte)
                    .with_replaces(vec!["eval.binary.lte".to_string()]),
            )
            .expect("override binary.lte eval scheme");

        lang
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::Term;

    /// The canonical base language, assembled once from the dev-dep data
    /// crate (tests may depend on `lang/` data; the library must not).
    fn canonical() -> &'static Language {
        static LANG: std::sync::OnceLock<Language> = std::sync::OnceLock::new();
        LANG.get_or_init(|| {
            handl_kit_data_prelude::language_from_yaml()
                .expect("canonical language assembles")
        })
    }

    fn arrow_language() -> Language {
        arrow_language_for(canonical())
    }

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

    #[test]
    fn arrow_eval_overrides_exist() {
        // The arrow `Language` overrides the canonical runtime-oriented
        // eval schemes for `flow` and `binary.lt`/`binary.lte` with
        // arrow-calculus `Term` templates.
        let lang = arrow_language();

        let flow = lang.eval.get_by_identity("flow").expect("flow eval");
        assert!(
            matches!(flow.scheme, Term::Seq { .. }),
            "arrow eval.flow should be a Seq scheme, got {:?}",
            flow.scheme
        );

        let bin_lt = lang
            .eval
            .get_by_identity("binary.lt")
            .expect("binary.lt eval");
        assert!(
            matches!(bin_lt.scheme, Term::App { .. }),
            "arrow eval.binary.lt should be an App scheme, got {:?}",
            bin_lt.scheme
        );

        let bin_lte = lang
            .eval
            .get_by_identity("binary.lte")
            .expect("binary.lte eval");
        assert!(
            matches!(bin_lte.scheme, Term::App { .. }),
            "arrow eval.binary.lte should be an App scheme, got {:?}",
            bin_lte.scheme
        );
    }
}
