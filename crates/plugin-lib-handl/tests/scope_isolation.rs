//! Regression: a callee's `do`-block bindings must not leak into (or mutate)
//! the caller's live scope. The bytecode VM shares one `ScopeStack` across
//! `apply_app` calls; without a lexical floor, `t :: ...` inside a callee
//! body could overwrite a same-named binding the caller had in scope (here,
//! a match-pattern binding) — the caller then read the callee's value.

use handl_platform::{EvalContext, Module};
use handl_plugin_lib_handl::HandlModule;


/// Install the canonical default language once per test binary.
///
/// `HandlModule` bridge runtimes resolve [`default_language`] lazily
/// (ADR-0128 §"The 0.8 seam"); tests assemble it from the dev-dep data
/// crate (dev-deps may link `lang/` data; the library must not).
fn ensure_lang() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(
            handl_kit_data_canonical::language_from_yaml()
                .expect("canonical language assembles"),
        );
    }
}

const SRC: &str = r#"do {
  inner := s ~> do {
    t := s + "!"
    { text: t }
  }
  f := xs ~> match xs {
    []: "empty",
    [h, ...t]: do {
      q := inner(h)
      t
    }
  }
  { f: f }
}"#;

#[test]
fn callee_bindings_do_not_leak_into_caller() {
        ensure_lang();
    for eng in ["tree-walk", "bytecode"] {
        let m = HandlModule::new_raw("t", SRC, &["f"]).with_eval_engine(eng);
        let r = m
            .function("f")
            .unwrap()
            .call(&[serde_json::json!(["a", "b"])], &mut EvalContext::new())
            .unwrap();
        assert_eq!(
            r,
            serde_json::json!(["b"]),
            "{eng}: caller's `t` was shadowed by the callee's"
        );
    }
}
