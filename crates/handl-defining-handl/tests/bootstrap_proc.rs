//! # bootstrap_proc.rs — ADR-0052 Phase D proc-notation bootstrap test
//!
//! Tests that the **proc-authored** bootstrap source file
//! (`data/handl-defining-handl-proc.hnl`) evaluates to the same result as
//! the v1 bootstrap (`data/handl-defining-handl.hnl`): `3`.
//!
//! The proc-authored version wraps the `eval(self, parse(self, lex(self,
//! "1 + 2")))` pipeline in a `proc lang -> ...` and applies it to `self`
//! via pipe: `self | proc lang -> eval(lang, parse(lang, lex(lang, "1 +
//! 2")))`. This demonstrates the arrow-calculus authoring surface
//! (`proc x -> ...`) in the bootstrap itself.

use handl_kit_canonical::Kit;

/// The proc-authored bootstrap (`self | proc lang -> eval(lang, parse(lang,
/// lex(lang, "1 + 2")))`) evaluates to `3`, matching the v1 bootstrap.
#[test]
fn proc_bootstrap_self_host() {
    let source = include_str!("../data/handl-defining-handl-proc.hnl");
    let kit = Kit::new().expect("kit must assemble");
    let result = kit.run(source).expect("proc bootstrap must succeed");
    assert_eq!(
        result,
        serde_json::json!(3),
        "proc-authored bootstrap should return 3, got: {}",
        result,
    );
}
