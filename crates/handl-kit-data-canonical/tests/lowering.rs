//! `lower_eval_yaml`: lower every scheme in the canonical `eval.yaml` via
//! `lower_v1_to_v2` (ADR-0052) and assert the v2 shapes.
//!
//! Every eval scheme lowers successfully except `eval.vau` (node_kind `vau`),
//! which must error because `Vau` is removed from the v2 kernel. A few
//! representative schemes are checked for their exact lowered shape, and
//! `foreign_names_v2` is exercised over a native and a foreign scheme.

use std::collections::BTreeSet;

use handl_kit_data_canonical;
use handl_language::{
    ArrSource, EvalSchemeDef, Language, SchemeV2, foreign_names_v2, lower_v1_to_v2,
};
use serde_json::Value;

/// Assemble the canonical kit and return its language.
fn canonical_language() -> Language {
    let kit = handl_kit_data_canonical::assemble().expect("canonical kit assembles");
    kit.lang().clone()
}

/// Look up an eval scheme definition by id.
fn eval_by_id<'a>(lang: &'a Language, id: &str) -> &'a EvalSchemeDef {
    lang.eval
        .fragments()
        .find(|d| d.id == id)
        .unwrap_or_else(|| panic!("eval fragment {id} should be present"))
}

#[test]
fn eval_fragment_count_matches_yaml() {
    // `eval.yaml` contains 53 `- id:` entries.
    let lang = canonical_language();
    assert_eq!(lang.eval.len(), 53);
}

#[test]
fn every_eval_scheme_lowers_except_vau() {
    let lang = canonical_language();
    let mut ok = 0usize;
    let mut vau_error: Option<String> = None;
    for def in lang.eval.fragments() {
        let res = lower_v1_to_v2(&def.scheme);
        if def.node_kind == "vau" {
            match res {
                Err(e) => {
                    let msg = format!("{e:?}");
                    assert!(
                        msg.contains("Vau"),
                        "vau error should mention Vau: {msg}"
                    );
                    vau_error = Some(msg);
                }
                Ok(_) => panic!("eval.vau should not lower successfully"),
            }
        } else {
            res.unwrap_or_else(|e| panic!("{} should lower: {e:?}", def.id));
            ok += 1;
        }
    }
    // 53 total - 1 vau = 52 successful lowerings.
    assert_eq!(ok, 52);
    assert!(vau_error.is_some(), "eval.vau should have errored");
}

#[test]
fn eval_binary_add_lowers_to_native_add_arr() {
    let lang = canonical_language();
    let def = eval_by_id(&lang, "eval.binary.add");
    let v2 = lower_v1_to_v2(&def.scheme).unwrap();
    assert_eq!(
        v2,
        SchemeV2::Arr {
            op: "add".into(),
            args: vec![
                SchemeV2::arr_native_lit("proj_child", vec![], Value::from(0u64)),
                SchemeV2::arr_native_lit("proj_child", vec![], Value::from(1u64)),
            ],
            source: ArrSource::Native,
            lit: None,
        }
    );
}

#[test]
fn eval_if_lowers_to_tag_choice() {
    let lang = canonical_language();
    let def = eval_by_id(&lang, "eval.if");
    let v2 = lower_v1_to_v2(&def.scheme).unwrap();
    // cond(Child 0) >>> tag >>> Choice(Child 1, Child 2)
    assert_eq!(
        v2,
        SchemeV2::seq(
            SchemeV2::seq(
                SchemeV2::arr_native_lit("proj_child", vec![], Value::from(0u64)),
                SchemeV2::arr_native("tag", vec![]),
            ),
            SchemeV2::choice(
                SchemeV2::arr_native_lit("proj_child", vec![], Value::from(1u64)),
                SchemeV2::arr_native_lit("proj_child", vec![], Value::from(2u64)),
            ),
        )
    );
}

#[test]
fn eval_let_lowers_to_seq_seq_extend_env_body() {
    let lang = canonical_language();
    let def = eval_by_id(&lang, "eval.let");
    let v2 = lower_v1_to_v2(&def.scheme).unwrap();
    // value(Child 0) >>> extend_env("$payload") >>> body(Child 1)
    assert_eq!(
        v2,
        SchemeV2::seq(
            SchemeV2::seq(
                SchemeV2::arr_native_lit("proj_child", vec![], Value::from(0u64)),
                SchemeV2::arr_native_lit("extend_env", vec![], Value::String("$payload".into())),
            ),
            SchemeV2::arr_native_lit("proj_child", vec![], Value::from(1u64)),
        )
    );
}

#[test]
fn eval_call_lowers_to_app() {
    let lang = canonical_language();
    let def = eval_by_id(&lang, "eval.call");
    let v2 = lower_v1_to_v2(&def.scheme).unwrap();
    // App { callee: Child 0, args: [Child 1, Child 2] }
    assert_eq!(
        v2,
        SchemeV2::app(
            SchemeV2::arr_native_lit("proj_child", vec![], Value::from(0u64)),
            vec![
                SchemeV2::arr_native_lit("proj_child", vec![], Value::from(1u64)),
                SchemeV2::arr_native_lit("proj_child", vec![], Value::from(2u64)),
            ],
        )
    );
}

#[test]
fn foreign_names_v2_empty_over_native_add() {
    let lang = canonical_language();
    let def = eval_by_id(&lang, "eval.binary.add");
    let v2 = lower_v1_to_v2(&def.scheme).unwrap();
    let names: BTreeSet<String> = foreign_names_v2(&v2);
    assert!(names.is_empty());
}

#[test]
fn foreign_names_v2_over_fanout_yields_fn_name() {
    let lang = canonical_language();
    let def = eval_by_id(&lang, "eval.fanout");
    let v2 = lower_v1_to_v2(&def.scheme).unwrap();
    let names: BTreeSet<String> = foreign_names_v2(&v2);
    let expected: BTreeSet<String> = ["fn.fanout_fn".into()].into_iter().collect();
    assert_eq!(names, expected);
}
