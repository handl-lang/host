use std::fs;
use std::path::PathBuf;

use handl_plugin_sdkgen::*;
use handl_plugin_sdkgen_emitter_file::FileEmitter;

fn build_clock_module() -> SdkgenModule {
    let mut module = SdkgenModule::default();
    module.metadata = SdkgenMetadata {
        module_name: "clock".to_string(),
        backend_id: "c".to_string(),
        target: "linux-x86_64".to_string(),
        client_package: "libc".to_string(),
        foreign_so_name: "libc.so.6".to_string(),
    };

    module.records.push(SdkgenRecord {
        name: "Timespec".to_string(),
        fields: vec![
            SdkgenRecordField {
                name: "tv_sec".to_string(),
                port_type: "Int".to_string(),
                descriptor: "i64".to_string(),
            },
            SdkgenRecordField {
                name: "tv_nsec".to_string(),
                port_type: "Int".to_string(),
                descriptor: "i64".to_string(),
            },
        ],
        size: 16,
        align: 8,
        doc: Some("C `struct timespec`.".to_string()),
    });

    module.functions.push(SdkgenFunction {
        name: "ClockGettime".to_string(),
        c_name: Some("clock_gettime".to_string()),
        doc: "`clock_gettime` port.\n@param {clock_id}\n@returns {Result<Timespec, Errno>}"
            .to_string(),
        args: vec![
            SdkgenFunctionArg {
                name: "clock_id".to_string(),
                port_type: "Int".to_string(),
                descriptor: "i32".to_string(),
                is_out: false,
            },
            SdkgenFunctionArg {
                name: "tp".to_string(),
                port_type: "Timespec".to_string(),
                descriptor: "out_struct:{tv_sec:i64,tv_nsec:i64}".to_string(),
                is_out: true,
            },
        ],
        ret: SdkgenReturn {
            port_type: "Result<Timespec, Errno>".to_string(),
            descriptor: "i32".to_string(),
        },
        variadic: false,
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
    assert!(
        adapter.contains("export clock_gettime: ClockGettime :="),
        "adapter missing: {adapter}"
    );
    assert!(adapter.contains("ffi.call("), "ffi.call missing: {adapter}");
    assert!(
        adapter.contains("result.err(ffi.errno())"),
        "result.err missing: {adapter}"
    );
    assert!(
        adapter.contains("result.ok(r.buf)"),
        "result.ok missing: {adapter}"
    );

    let _ = fs::remove_dir_all(&tmp);
}
