//! Verifies that the `str` abstraction contract (the `lib-str` package's
//! `src/index.hnl`, vendored into `OUT_DIR` by build.rs — T-044) parses
//! cleanly with `parse_contracts` and registers all 45 axioms.

use handl_plugin_ext_refinement::parse_contracts;

const STR_HNL: &str = include_str!(concat!(env!("OUT_DIR"), "/lib-str/src/index.hnl"));

#[test]
fn str_contract_parses_all_45_axioms() {
    let src = STR_HNL;
    let reg = parse_contracts(src).expect("str.hnl must parse");
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
/// The package lives in the `lib-char` repo; build.rs vendors it into
/// `OUT_DIR` (T-044 — previously `handl-plugin-lib-char-native::CHAR_HNL`).
const CHAR_HNL: &str = include_str!(concat!(env!("OUT_DIR"), "/lib-char/src/index.hnl"));

#[test]
fn char_contract_parses_all_axioms() {
    let src = CHAR_HNL;
    let reg = parse_contracts(src).expect("char.hnl must parse");
    assert_eq!(
        reg.len(),
        18,
        "char contract must declare exactly 18 axioms"
    );

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
