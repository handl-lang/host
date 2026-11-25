//! `hnl_ext_atomfix` — a minimal cdylib **atom provider** fixture
//! (P-013 phase-2c, proposal-0012).
//!
//! The manifest carries `atoms_yaml`, marking the library as an atom
//! provider; atom calls arrive through the ordinary
//! `handl_ext_dispatch` entry point with `cmd` = the qualified atom name
//! and `args[0]` = a JSON array of call args (see `handl_ext::atoms`).
//! `handl-plugin-lib-handl`'s dep binding discovers this `.so` through
//! the ADR-0149 search path and binds dep `atoms.yaml` specs to it —
//! the same registry path statically linked providers take, minus the
//! Cargo dep edge.

use handl_ext::{ExtRequest, ExtResponse, ExtensionManifest};
use serde_json::{Value, json};

/// The provider's own atom spec — the claim surface hosts parse for
/// module names. Module `atomfix`:
///
/// - `atomfix.echo : String ~> String` — returns `arg ++ "!"`.
/// - `atomfix.add : (Int * Int) ~> Int` — returns the sum.
/// - `atomfix.fail` — always fails (`code: 1`, `stderr: "…boom…"`).
const ATOMS_YAML: &str = r#"
module: atomfix
atoms:
  - { name: atomfix.echo, sig: "String ~> String", impl: atom_echo, total: true }
  - { name: atomfix.add, sig: "(Int * Int) ~> Int", impl: atom_add, total: true }
  - { name: atomfix.fail, impl: atom_fail }
"#;

handl_ext::export!(
    ExtensionManifest::new("atomfix")
        .version(env!("CARGO_PKG_VERSION"))
        .atoms_yaml(ATOMS_YAML),
    dispatch
);

/// Atom dispatch: `cmd` is the qualified atom name; `req.args[0]` is the
/// JSON args array (`handl_ext::atom_call_args`).
fn dispatch(cmd: &str, req: &ExtRequest) -> ExtResponse {
    match cmd {
        "atomfix.echo" => {
            let args = match handl_ext::atom_call_args(req) {
                Ok(a) => a,
                Err(r) => return r,
            };
            match args.first().and_then(Value::as_str) {
                Some(s) => handl_ext::atom_result(&json!(format!("{s}!"))),
                None => handl_ext::atom_failure("atomfix.echo: expected a string arg"),
            }
        }
        "atomfix.add" => {
            let args = match handl_ext::atom_call_args(req) {
                Ok(a) => a,
                Err(r) => return r,
            };
            let (Some(a), Some(b)) = (
                args.first().and_then(Value::as_f64),
                args.get(1).and_then(Value::as_f64),
            ) else {
                return handl_ext::atom_failure("atomfix.add: expected two numbers");
            };
            let sum = a + b;
            let out = if sum.fract() == 0.0 {
                json!(sum as i64)
            } else {
                json!(sum)
            };
            handl_ext::atom_result(&out)
        }
        "atomfix.fail" => handl_ext::atom_failure("atomfix.fail: boom"),
        other => ExtResponse::error(2, format!("atomfix: unknown atom `{other}`\n")),
    }
}
