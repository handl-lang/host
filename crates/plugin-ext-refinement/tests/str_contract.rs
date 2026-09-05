//! Verifies that `handl-packages/str/str.hnl` (the str abstraction contract)
//! parses cleanly with `parse_contracts` and registers all 45 axioms.

use std::path::PathBuf;

use handl_plugin_ext_refinement::parse_contracts;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn str_contract_parses_all_45_axioms() {
    let path = workspace_root().join("handl-packages/str/str.hnl");
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e));
    let reg = parse_contracts(&src).expect("str.hnl must parse");
    assert_eq!(reg.len(), 45, "str contract must declare exactly 45 axioms");

    // Spot-check a few representative axioms are registered under their
    // canonical names.
    let names: Vec<String> = reg.iter().map(|(n, _, _)| n.clone()).collect();
    for required in [
        "str.upper",
        "str.trim",
        "str.reverse",
        "str.replace",
        "str.levenshtein",
        "str.slugify",
        "str.word_wrap",
        "str.split_at",
    ] {
        assert!(
            names.iter().any(|n| n == required),
            "missing axiom `{}` in str contract",
            required
        );
    }
}

/// The `char` contract is the semantic floor beneath `str`: category,
/// case, scalar, and char-sequence atoms that every backend
/// (`char-native`, `char-ascii`, `char-js`, `char-icu`) implements.
#[test]
fn char_contract_parses_all_axioms() {
    let path = workspace_root().join("handl-packages/char/char.hnl");
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e));
    let reg = parse_contracts(&src).expect("char.hnl must parse");
    assert_eq!(reg.len(), 18, "char contract must declare exactly 18 axioms");

    let names: Vec<String> = reg.iter().map(|(n, _, _)| n.clone()).collect();
    for required in [
        "char.is_whitespace",
        "char.is_alpha",
        "char.is_alnum",
        "char.to_upper",
        "char.to_lower",
        "char.code_point",
        "char.from_code_point",
        "char.at",
        "char.slice",
        "char.length",
        "char.concat",
        "char.chars",
        "char.from_chars",
    ] {
        assert!(
            names.iter().any(|n| n == required),
            "missing axiom `{}` in char contract",
            required
        );
    }
}
