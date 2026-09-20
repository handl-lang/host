use std::fs;
use std::path::PathBuf;

use handl_plugin_sdkgen::*;
use handl_plugin_sdkgen_emitter_file::FileEmitter;

fn build_clock_module() -> SdkgenModule {
    let mut module = SdkgenModule::default();
    module.module = "clock".to_string();
    module.backend_id = "c".to_string();
    module.target = "x86_64-unknown-linux".to_string();
    module.client_package = "libc".to_string();
    module.library = "libc.so.6".to_string();

    module.records.push(Record {
        name: "Timespec".to_string(),
        doc: None,
        fields: vec![
            Field {
                name: "tv_sec".to_string(),
                port: "Int".to_string(),
                descriptor: "i64".to_string(),
                offset: 0,
                size: 8,
                ptr_depth: 0,
                is_const: false,
                doc: None,
            },
            Field {
                name: "tv_nsec".to_string(),
                port: "Int".to_string(),
                descriptor: "i64".to_string(),
                offset: 8,
                size: 8,
                ptr_depth: 0,
                is_const: false,
                doc: None,
            },
        ],
        size: 16,
        align: 8,
    });

    module.functions.push(Function {
        name: "clock_gettime".to_string(),
        doc: None,
        port_name: "ClockGettime".to_string(),
        port_args: vec![
            Arg {
                name: "clock_id".to_string(),
                port: "Int".to_string(),
            },
            Arg {
                name: "tp".to_string(),
                port: "Timespec".to_string(),
            },
        ],
        arg_descriptors: vec![
            "i32".to_string(),
            "out_struct:{tv_sec:i64@0,tv_nsec:i64@8}".to_string(),
        ],
        return_port: "ClockGettimeResult".to_string(),
        ret_descriptor: "i32".to_string(),
        out_struct: Some("Timespec".to_string()),
        variadic: false,
        c_decl: "int clock_gettime(clockid_t clock_id, struct timespec *tp);".to_string(),
    });

    module
}

#[test]
fn emits_port_and_adapter() {
    let module = build_clock_module();
    let tmp = PathBuf::from(format!("/tmp/sdkgen_emit_test_{}", std::process::id()));
    let port_dir = tmp.join("libc");

    let _ = fs::remove_dir_all(&tmp);
    FileEmitter::new().emit(&module, &port_dir).unwrap();

    let port = fs::read_to_string(port_dir.join("clock.hnl")).unwrap();
    let adapter = fs::read_to_string(tmp.join("libc-c-linux-x86_64/clock.hnl")).unwrap();

    assert!(
        port.contains("export Timespec := {"),
        "port record missing: {port}"
    );
    assert!(
        port.contains("export ClockGettime:"),
        "port arrow missing: {port}"
    );
    // out_struct keeps `ret` — the port declares a Result record whose
    // `buf` is the struct, and the adapter returns it whole.
    assert!(
        port.contains("export ClockGettimeResult := {"),
        "result record missing: {port}"
    );
    assert!(
        port.contains("buf: Timespec"),
        "result buf field missing: {port}"
    );
    assert!(
        adapter.contains("export clock_gettime: ClockGettime :="),
        "adapter missing: {adapter}"
    );
    assert!(
        adapter.contains("call(\"libc.so.6\""),
        "call missing: {adapter}"
    );
    assert!(
        adapter.contains("err(errno())"),
        "err(errno()) missing: {adapter}"
    );
    assert!(adapter.contains("ok(r)"), "ok(r) missing: {adapter}");

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn out_buf_descriptor_gets_buf_unwrap_body() {
    let mut module = SdkgenModule::default();
    module.module = "socket".to_string();
    module.backend_id = "c".to_string();
    module.target = "x86_64-unknown-linux".to_string();
    module.client_package = "libc".to_string();
    module.library = "libc.so.6".to_string();

    module.functions.push(Function {
        name: "recv".to_string(),
        doc: None,
        port_name: "Recv".to_string(),
        port_args: vec![
            Arg {
                name: "fd".to_string(),
                port: "Int".to_string(),
            },
            Arg {
                name: "flags".to_string(),
                port: "Int".to_string(),
            },
        ],
        arg_descriptors: vec![
            "i32".to_string(),
            "out_buf:65536".to_string(),
            "i32".to_string(),
        ],
        return_port: "Obj".to_string(),
        ret_descriptor: "i64".to_string(),
        out_struct: None,
        variadic: false,
        c_decl: "ssize_t recv(int, void *, size_t, int);".to_string(),
    });

    let tmp = PathBuf::from(format!("/tmp/sdkgen_emit_outbuf_{}", std::process::id()));
    let port_dir = tmp.join("libc");

    let _ = fs::remove_dir_all(&tmp);
    FileEmitter::new().emit(&module, &port_dir).unwrap();

    let adapter = fs::read_to_string(tmp.join("libc-c-linux-x86_64/socket.hnl")).unwrap();
    assert!(
        adapter.contains("\"out_buf:65536\""),
        "out_buf descriptor missing: {adapter}"
    );
    assert!(
        adapter.contains("if r.ret < 0"),
        "r.ret check missing: {adapter}"
    );
    assert!(
        adapter.contains("ok(r.buf)"),
        "ok(r.buf) missing: {adapter}"
    );

    let _ = fs::remove_dir_all(&tmp);
}
