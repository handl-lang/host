use handl_plugin_sdkgen::*;

#[test]
fn serde_round_trip() {
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
        doc: "Read a POSIX clock.\n@param {clock_id}\n@returns {Result<Timespec, Errno>}"
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

    module.overrides.insert(
        "clock_gettime".to_string(),
        SdkgenOverride::new().hint("errno".to_string(), "true".to_string()),
    );

    let json = serde_json::to_string(&module).unwrap();
    let round: SdkgenModule = serde_json::from_str(&json).unwrap();

    assert_eq!(round.metadata.module_name, "clock");
    assert_eq!(round.records[0].fields.len(), 2);
    assert_eq!(
        round.functions[0].args[1].descriptor,
        "out_struct:{tv_sec:i64,tv_nsec:i64}"
    );
    assert_eq!(round.overrides["clock_gettime"].hints["errno"], "true");
}
