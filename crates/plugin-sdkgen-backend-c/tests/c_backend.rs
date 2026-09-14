use std::path::Path;

#[test]
fn parses_simple_header() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest.join("tests/fixtures/simple.h");
    let backend = handl_plugin_sdkgen_backend_c::CBackend::new(None);
    let module = backend.parse_header(&path).expect("parse header");

    let rec = module
        .records
        .iter()
        .find(|r| r.name == "point")
        .expect("point record");
    assert_eq!(rec.fields.len(), 2);
    assert_eq!(rec.fields[0].descriptor, "i32");
    assert_eq!(rec.fields[1].descriptor, "i64");

    let add = module
        .functions
        .iter()
        .find(|f| f.name == "add")
        .expect("add function");
    assert_eq!(add.port_name, "Add");
    assert_eq!(add.port_args.len(), 2);
    assert_eq!(add.return_port, "Int");
    assert_eq!(add.arg_descriptors, &["i32", "i32"]);

    let mv = module
        .functions
        .iter()
        .find(|f| f.name == "move_point")
        .expect("move_point function");
    assert_eq!(mv.port_name, "MovePoint");
    assert_eq!(mv.out_struct, Some("point".into()));
    assert_eq!(mv.port_args.len(), 2);
    assert!(mv.arg_descriptors.iter().any(|d| d.starts_with("out_struct")));
}
