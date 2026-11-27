//! # json_handl.rs — `lib-json-handl` import-surface smoke test.
//!
//! `lib-json-handl`'s `json.hnl` is a pure-`:=` module: every function is
//! a plain top-level binding and the `data`-family ctor names can't cross
//! `import` (G5). Its public surface is declared with bare
//! `export <name>[, <name>…]` re-export lists — the same surface
//! `export sort, cons` uses in arrow modules (ADR-0069) — resolved by
//! `module_prep` against the module's `:=` bindings.
//!
//! This test exercises the consumer side through `Kit::eval` — the same
//! path `hnl run`/`hnl test` uses: star imports bind *only* the declared
//! exports (I-007), named `{ … }` imports project them, and internals
//! (`tok_step`, the `test_*` suite, the family records) stay file-local.
//!
//! NOTE: module eval under the term interpreter is slow for a ~1100-line
//! module, so every check runs inside ONE `kit.eval` — the module is
//! loaded once (same pattern as `intl_c.rs`).

use handl_kit_prelude::Kit;
use serde_json::{Value, json};
use std::sync::OnceLock;

/// One `Kit::eval` that loads `json.hnl` once and runs every check,
/// returning a `{ name: result }` record.
fn results() -> &'static Value {
    static RESULTS: OnceLock<Value> = OnceLock::new();
    RESULTS.get_or_init(|| {
        let kit = Kit::new().expect("kit must build");
        let src = r#"do {
  from "../../../lib-json-handl/src/json.hnl" import { parse, toStr }
  from "../../../lib-json-handl/src/json.hnl" import * as j
  {
    -- named imports project the declared exports
    named_roundtrip: do { r := parse("{\"a\": [1]}"); if j.is_ok(r) { toStr(r.v) } else { "ERR" } },
    -- star import binds the export namespace
    star_ok: j.is_ok(j.parse("[1, 2]")),
    star_parse: j.is_ok(j.parse("true")),
    star_bad: j.is_err(j.parse("nul")),
    -- the `smoke` alias export and the `mk_*` factories
    star_smoke: j.smoke,
    star_mk: j.toStr(j.mk_jarr([j.mk_jnum(1)])),
    -- internals are NOT exported: `tok_step` is a private `:=` binding
    internal_tok_step: j.tok_step
  }
}
"#;
        kit.eval(src).expect("eval must succeed")
    })
}

#[test]
fn json_handl_named_and_star_imports() {
    let r = results();
    assert_eq!(r.get("named_roundtrip"), Some(&json!("{\"a\":[1]}")), "{r}");
    assert_eq!(r.get("star_ok"), Some(&json!(true)), "{r}");
    assert_eq!(r.get("star_parse"), Some(&json!(true)), "{r}");
    assert_eq!(r.get("star_bad"), Some(&json!(true)), "{r}");
    assert!(r.get("star_smoke").is_some(), "{r}");
    assert_eq!(r.get("star_mk"), Some(&json!("[1]")), "{r}");
    // `tok_step` is internal — not a declared export, so the star-import
    // record must not carry it (absent field → `null`/missing).
    let internal = r.get("internal_tok_step");
    assert!(
        internal.is_none() || internal == Some(&Value::Null),
        "internal name leaked into export surface: {internal:?}"
    );
}
