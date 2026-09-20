use handl_plugin_sdkgen::*;

#[test]
fn serde_round_trip() {
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
            "out_struct:{tv_sec:i64,tv_nsec:i64}".to_string(),
        ],
        return_port: "Result<Timespec, Errno>".to_string(),
        ret_descriptor: "i32".to_string(),
        out_struct: Some("out_struct:{tv_sec:i64,tv_nsec:i64}".to_string()),
        variadic: false,
        c_decl: "int clock_gettime(clockid_t clock_id, struct timespec *tp);".to_string(),
    });

    let json = serde_json::to_string(&module).unwrap();
    let round: SdkgenModule = serde_json::from_str(&json).unwrap();

    assert_eq!(round.module, "clock");
    assert_eq!(round.records[0].fields.len(), 2);
    assert_eq!(
        round.functions[0].arg_descriptors[1],
        "out_struct:{tv_sec:i64,tv_nsec:i64}"
    );
}
