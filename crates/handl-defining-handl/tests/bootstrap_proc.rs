//! # bootstrap_proc.rs — ADR-0052 Phase D arrow-notation bootstrap test
//!
//! Tests that the **arrow-authored** bootstrap source file
//! (`data/handl-defining-handl-proc.hnl`) evaluates to the same result as
//! the v1 bootstrap (`data/handl-defining-handl.hnl`): `3`.
//!
//! The arrow-authored version wraps the `eval(self, parse(self, lex(self,
//! "1 + 2")))` pipeline in a `lang ~> ...` and applies it to `self`
//! via pipe: `self |> lang ~> eval(lang, parse(lang, lex(lang, "1 +
//! 2")))`. This demonstrates the arrow-calculus authoring surface
//! (`x ~> body`) in the bootstrap itself.

use handl_kit_canonical::Kit;

/// The arrow-authored bootstrap (`self |> lang ~> eval(lang, parse(lang,
/// lex(lang, "1 + 2")))`) evaluates to `3`, matching the v1 bootstrap.
#[ignore = "v0.1 removed |> and ~> surface syntax (ADR-0074); rewrite using >>> flow or direct application"]
#[test]
fn proc_bootstrap_self_host() {
    let source = include_str!("../data/handl-defining-handl-proc.hnl");
    let kit = Kit::new().expect("kit must assemble");
    let result = kit.run(source).expect("arrow bootstrap must succeed");
    assert_eq!(
        result,
        serde_json::json!(3),
        "arrow-authored bootstrap should return 3, got: {}",
        result,
    );
}
