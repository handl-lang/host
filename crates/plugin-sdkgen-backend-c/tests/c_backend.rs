use handl_plugin_sdkgen::SdkgenBackend;
use handl_plugin_sdkgen_backend_c::CBackend;
use std::path::Path;

#[test]
fn parses_simple_header() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest.join("tests/fixtures/simple.h");
    let backend = CBackend::new(None, Default::default(), None, None).unwrap();
    let module = backend.parse_header(&path).expect("parse header");

    let rec = module
        .records
        .iter()
        .find(|r| r.name == "point")
        .expect("point record");
    assert_eq!(rec.fields.len(), 2);
    assert_eq!(rec.fields[0].descriptor, "i32");
    assert_eq!(rec.fields[1].descriptor, "i64");
    assert!(rec.doc.as_ref().unwrap().contains("2D"));

    let add = module
        .functions
        .iter()
        .find(|f| f.name == "add")
        .expect("add function");
    assert_eq!(add.port_name, "Add");
    assert_eq!(add.port_args.len(), 2);
    assert_eq!(add.return_port, "Int");
    assert_eq!(add.arg_descriptors, &["i32", "i32"]);
    assert!(add.doc.as_ref().unwrap().contains("Add two"));

    let mv = module
        .functions
        .iter()
        .find(|f| f.name == "move_point")
        .expect("move_point function");
    assert_eq!(mv.port_name, "MovePoint");
    assert_eq!(mv.out_struct, Some("point".into()));
    assert_eq!(mv.port_args.len(), 2);
    assert!(
        mv.arg_descriptors
            .iter()
            .any(|d| d.starts_with("out_struct"))
    );
}

#[test]
fn size_pair_collapses_to_bytes_arg() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest.join("tests/fixtures/simple.h");
    let backend = CBackend::new(None, Default::default(), None, None).unwrap();
    let module = backend.parse_header(&path).expect("parse header");

    // sock_send(fd, const void *buf, size_t len, int flags): the (ptr, len)
    // pair collapses to one Bytes arg backed by an `in_buf` descriptor that
    // expands back to two C args at marshal time.
    let f = module
        .functions
        .iter()
        .find(|f| f.name == "sock_send")
        .expect("sock_send function");
    assert_eq!(f.port_args.len(), 3);
    assert_eq!(f.port_args[0].name, "fd");
    assert_eq!(f.port_args[1].name, "buf");
    assert_eq!(f.port_args[1].port, "Bytes");
    assert_eq!(f.port_args[2].name, "flags");
    assert_eq!(f.arg_descriptors, &["i32", "in_buf", "i32"]);
}

#[test]
fn const_record_ptr_with_len_collapses_to_bytes_arg() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest.join("tests/fixtures/simple.h");
    let backend = CBackend::new(None, Default::default(), None, None).unwrap();
    let module = backend.parse_header(&path).expect("parse header");

    // sock_connect(fd, const struct sockaddr *addr, socklen_t addrlen):
    // `socklen_t` is an integer typedef, not `size_t` — still a length.
    let f = module
        .functions
        .iter()
        .find(|f| f.name == "sock_connect")
        .expect("sock_connect function");
    assert_eq!(f.port_args.len(), 2);
    assert_eq!(f.port_args[1].name, "addr");
    assert_eq!(f.port_args[1].port, "Bytes");
    assert_eq!(f.arg_descriptors, &["i32", "in_buf"]);

    // sock_setopt(fd, level, name, const void *optval, socklen_t optlen)
    let f = module
        .functions
        .iter()
        .find(|f| f.name == "sock_setopt")
        .expect("sock_setopt function");
    assert_eq!(f.port_args.len(), 4);
    assert_eq!(f.port_args[3].name, "optval");
    assert_eq!(f.arg_descriptors, &["i32", "i32", "i32", "in_buf"]);
}

#[test]
fn macro_constants_and_anonymous_enums() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest.join("tests/fixtures/simple.h");
    let backend = CBackend::new(None, Default::default(), None, None).unwrap();
    let module = backend.parse_header(&path).expect("parse header");

    // Object-like macros are evaluated into constants...
    let c = module
        .enum_constants
        .iter()
        .find(|c| c.name == "TEST_AF_INET")
        .expect("TEST_AF_INET constant");
    assert_eq!(c.value, 2);
    // ...function-like macros are not.
    assert!(!module.enum_constants.iter().any(|c| c.name == "TEST_MAX"));
    // Anonymous enum constants are collected.
    assert!(
        module
            .enum_constants
            .iter()
            .any(|c| c.name == "ANON_ONE" && c.value == 1)
    );
}

#[test]
fn include_filter_matches_anonymous_enum_constants() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest.join("tests/fixtures/simple.h");
    // The anonymous enum has no name for `include` to match; its constants
    // must still be reachable by their own names.
    let backend = CBackend::new(None, Default::default(), Some("ANON_*".into()), None).unwrap();
    let module = backend.parse_header(&path).expect("parse header");
    assert!(
        module
            .enum_constants
            .iter()
            .any(|c| c.name == "ANON_TWO" && c.value == 2)
    );
}

#[test]
fn opaque_struct_ptr_falls_back_to_handle() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest.join("tests/fixtures/simple.h");
    let backend = CBackend::new(None, Default::default(), None, None).unwrap();
    let module = backend.parse_header(&path).expect("parse header");

    // `struct opaque_ctx` is forward-declared only: not a caller-allocated
    // out-struct, but an opaque handle passed through as a pointer.
    let f = module
        .functions
        .iter()
        .find(|f| f.name == "opaque_ping")
        .expect("opaque_ping function");
    assert_eq!(f.port_args.len(), 1);
    assert_eq!(f.port_args[0].port, "Handle");
    assert_eq!(f.arg_descriptors, &["ptr"]);
    assert!(f.out_struct.is_none());
}

#[test]
fn cstr_return_and_small_int_args() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest.join("tests/fixtures/simple.h");
    let backend = CBackend::new(None, Default::default(), None, None).unwrap();
    let module = backend.parse_header(&path).expect("parse header");

    let f = module
        .functions
        .iter()
        .find(|f| f.name == "sock_recv_str")
        .expect("sock_recv_str function");
    assert_eq!(f.return_port, "String");
    assert_eq!(f.ret_descriptor, "cstr");

    // unsigned short widens to i32 (small ints travel in full-width slots).
    let f = module
        .functions
        .iter()
        .find(|f| f.name == "sock_htons")
        .expect("sock_htons function");
    assert_eq!(f.arg_descriptors, &["i32"]);
    assert_eq!(f.ret_descriptor, "i32");
}

#[test]
fn bitfield_record_reports_diagnostic() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest.join("tests/fixtures/simple.h");
    let backend = CBackend::new(None, Default::default(), None, None).unwrap();
    let module = backend.parse_header(&path).expect("parse header");

    assert!(!module.records.iter().any(|r| r.name == "flags"));
    assert!(
        module
            .diagnostics
            .iter()
            .any(|d| d.contains("flags") && d.contains("bitfield"))
    );

    // `flags` failed to parse, so `struct flags*` is not a known out-struct:
    // it falls back to an opaque handle argument.
    let f = module
        .functions
        .iter()
        .find(|f| f.name == "flags_check")
        .expect("flags_check function");
    assert_eq!(f.arg_descriptors, &["ptr"]);
}

#[test]
fn only_filter_matches_functions() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest.join("tests/fixtures/simple.h");
    let backend = CBackend::new(None, Default::default(), Some("add".into()), None).unwrap();
    let module = backend.parse_header(&path).expect("parse header");
    assert_eq!(module.functions.len(), 1);
    assert_eq!(module.functions[0].name, "add");
    assert!(module.records.is_empty());
}

#[test]
fn only_filter_matches_records() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest.join("tests/fixtures/simple.h");
    let backend = CBackend::new(None, Default::default(), Some("point".into()), None).unwrap();
    let module = backend.parse_header(&path).expect("parse header");
    assert_eq!(module.records.len(), 1);
    assert_eq!(module.records[0].name, "point");
    assert!(module.functions.is_empty());
}
