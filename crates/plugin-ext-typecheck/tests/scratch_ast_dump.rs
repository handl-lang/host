//! SCRATCH (temporary): dump the parsed AST for ADR-0158 candidate sources.

use handl_kit_prelude::Kit;

fn dump(src: &str) {
    let kit = Kit::new().expect("kit assembles");
    println!("===== SOURCE =====\n{src}");
    match kit.parse_raw(src) {
        Ok(ast) => {
            for (i, n) in ast.nodes().iter().enumerate() {
                println!(
                    "{i}: kind={} span={:?} payload={}",
                    n.kind, n.span, n.payload
                );
            }
        }
        Err(e) => println!("PARSE ERROR: {e}"),
    }
    println!();
}

#[test]
fn scratch_dump() {
    dump("data Nat :: Zero | Succ(pred : Nat)");
    dump(
        "data Vec(n : Nat, a : Type) :: VNil : Vec(0, a) | VCons(x : a, xs : Vec(n, a)) : Vec(n + 1, a)",
    );
    dump("Nat := {\n  Zero : Nat,\n  Succ : (pred : Nat) ~> Nat\n}");
    dump(
        "Vec<n : Nat, a : Type> := {\n  VNil : Vec(0, a),\n  VCons : (x : a, xs : Vec(n, a)) ~> Vec(n + 1, a)\n}",
    );
    dump("data Eq(a : Type) :: mkEq(f : a ~> a ~> Bool)");
    dump("Opt<a : Type> := {\n  None : Opt,\n  Some : (x : a) ~> Opt\n}");
}
