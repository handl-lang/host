use handl_runtime::module_prep;

#[test]
fn probe_shapes() {
    let cases = [
        ("one-line-file", "from \"type\" import { Number } export f: T := (x) ~> do { if x > 0 { 1 } else { 0 } }\n"),
        ("indented", "  export f := (x) ~> do { if x > 0 { 1 } else { 0 } }\n"),
        ("two-exports-oneline", "export f := (x) ~> do { if x > 0 { 1 } else { 0 } } export g := (x) ~> x\n"),
        ("reported", "export clamp: Clamp := (x, lo, hi) ~> do { if x < lo { lo } else { if x > hi { hi } else { x } } }\n"),
    ];
    for (tag, src) in cases {
        let out = module_prep::prepare_module(src, &|s| s != "type", &|_| true);
        eprintln!("=== {tag} ===\n{out}\n");
    }
}
