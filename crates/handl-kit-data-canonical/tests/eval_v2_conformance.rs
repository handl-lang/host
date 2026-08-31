//! Conformance test: `eval_v2.yaml` is the exact v2 lowering of `eval.yaml`.
//!
//! For every non-`Vau` entry in the v1 `eval.yaml`, the corresponding entry in
//! `eval_v2.yaml` must equal `lower_v1_to_v2(&v1_scheme)`. This proves the v2
//! YAML is the faithful, pre-computed lowering of the v1 YAML (ADR-0052), so v2
//! engines can consume `eval_v2.yaml` directly without runtime v1->v2 lowering.
//!
//! `eval.vau` (node_kind `vau`) is intentionally absent from `eval_v2.yaml`
//! because `Vau` is removed from the v2 kernel (ADR-0052); it is a handler
//! concern, not a kernel combinator.

use handl_kit_data_canonical;
use handl_language::{EvalSchemeDefV2, Language, lower_v1_to_v2};

/// Assemble the canonical kit and return its language.
fn canonical_language() -> Language {
    let kit = handl_kit_data_canonical::assemble().expect("canonical kit assembles");
    kit.lang().clone()
}

#[test]
fn eval_v2_has_52_entries() {
    // 53 v1 entries minus the omitted `eval.vau` = 52 v2 entries.
    let lang = canonical_language();
    assert_eq!(lang.eval.len(), 53);
    assert_eq!(lang.eval_v2.len(), 52);
}

#[test]
fn eval_v2_yaml_is_exact_lowering_of_eval_yaml() {
    let lang = canonical_language();

    // Index v2 defs by id for direct comparison.
    let v2_by_id: std::collections::HashMap<&str, &EvalSchemeDefV2> = lang
        .eval_v2
        .fragments()
        .map(|d| (d.id.as_str(), d))
        .collect();

    for v1 in lang.eval.fragments() {
        if v1.node_kind == "vau" {
            // Vau has no v2 lowering; it must be absent from eval_v2.
            assert!(
                !v2_by_id.contains_key(v1.id.as_str()),
                "eval.vau should be omitted from eval_v2.yaml"
            );
            // And lowering it must error.
            assert!(
                lower_v1_to_v2(&v1.scheme).is_err(),
                "eval.vau should not lower"
            );
            continue;
        }

        let v2 = v2_by_id
            .get(v1.id.as_str())
            .unwrap_or_else(|| panic!("v2 entry for {} should be present", v1.id));

        // The v2 def must share the v1 def's id and node_kind.
        assert_eq!(v2.id, v1.id, "id mismatch for {}", v1.id);
        assert_eq!(v2.node_kind, v1.node_kind, "node_kind mismatch for {}", v1.id);

        // The v2 scheme must equal the lowering of the v1 scheme.
        let lowered = lower_v1_to_v2(&v1.scheme)
            .unwrap_or_else(|e| panic!("{} should lower: {e:?}", v1.id));
        assert_eq!(
            v2.scheme, lowered,
            "eval_v2.yaml scheme for {} does not match lower_v1_to_v2",
            v1.id
        );
    }
}

#[test]
fn eval_v2_get_by_identity_resolves_node_kind() {
    // The additive eval_v2 set supports the same get_by_identity(node_kind)
    // lookup pattern as the v1 eval set.
    let lang = canonical_language();
    let v2 = lang
        .eval_v2
        .get_by_identity("binary.add")
        .expect("binary.add should be in eval_v2");
    assert_eq!(v2.id, "eval.binary.add");

    // Cross-check: the v1 and v2 entries for the same node_kind agree on id.
    let v1 = lang
        .eval
        .get_by_identity("binary.add")
        .expect("binary.add should be in eval");
    assert_eq!(v1.id, v2.id);

    // vau is absent from eval_v2.
    assert!(lang.eval_v2.get_by_identity("vau").is_none());
}
