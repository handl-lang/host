use std::fs;
use std::path::PathBuf;

use handl_plugin_sdkgen::*;
use handl_plugin_sdkgen_emitter_file::FileEmitter;

fn build_console_module() -> SdkgenModule {
    let mut module = SdkgenModule::default();
    module.module = "console".to_string();
    module.backend_id = "webidl".to_string();
    module.target = "browser".to_string();
    module.client_package = "js".to_string();
    module.library = "console".to_string();
    module.source = Some("https://example.test/console.idl".to_string());

    module.functions.push(Function {
        name: "console_log".to_string(),
        doc: None,
        port_name: "ConsoleLog".to_string(),
        port_args: vec![Arg {
            name: "message".to_string(),
            port: "String".to_string(),
        }],
        arg_descriptors: vec!["js_string".to_string()],
        return_port: "()".to_string(),
        ret_descriptor: "void".to_string(),
        out_struct: None,
        variadic: false,
        c_decl: "console.log".to_string(),
    });

    module
}

#[test]
fn emits_js_port_and_adapter() {
    let module = build_console_module();
    let tmp = PathBuf::from(format!(
        "/tmp/sdkgen_emit_webidl_test_{}",
        std::process::id()
    ));
    let port_dir = tmp.join("console");

    let _ = fs::remove_dir_all(&tmp);
    FileEmitter::new().emit(&module, &port_dir).unwrap();

    let port = fs::read_to_string(port_dir.join("console.hnl")).unwrap();
    let adapter = fs::read_to_string(tmp.join("console-webidl-browser/console.hnl")).unwrap();

    assert!(
        port.contains("--- source: https://example.test/console.idl"),
        "port source pin missing: {port}"
    );
    assert!(
        adapter.contains("--- source: https://example.test/console.idl"),
        "adapter source pin missing: {adapter}"
    );
    assert!(
        port.contains("from \"js\" import { JsError }"),
        "port import missing: {port}"
    );
    assert!(
        port.contains("export ConsoleLog:"),
        "port arrow missing: {port}"
    );
    assert!(
        port.contains("~[1]~>"),
        "1-morphism missing in port: {port}"
    );
    assert!(
        port.contains("Result<(), JsError>"),
        "JS result type missing in port: {port}"
    );

    assert!(
        adapter.contains("from \"js\" import { call, error }"),
        "adapter import missing: {adapter}"
    );
    assert!(
        adapter.contains("export console_log: ConsoleLog :="),
        "adapter arrow missing: {adapter}"
    );
    assert!(
        adapter.contains("call(\"console\", \"log\""),
        "JS call missing: {adapter}"
    );
    assert!(
        adapter.contains("\"js_string\""),
        "js_string descriptor missing: {adapter}"
    );
    assert!(
        adapter.contains("\"js_void\""),
        "js_void descriptor missing: {adapter}"
    );
    assert!(
        adapter.contains("e = error()"),
        "error() call missing: {adapter}"
    );
    assert!(adapter.contains("err(e)"), "err(e) missing: {adapter}");

    // Emitted files are canonical: `hnl fmt --check` must pass on both.
    assert!(
        handl_plugin_ext_fmt::check_source(&port).unwrap_or(false),
        "port is not canonical:\n{port}"
    );
    assert!(
        handl_plugin_ext_fmt::check_source(&adapter).unwrap_or(false),
        "adapter is not canonical:\n{adapter}"
    );

    let _ = fs::remove_dir_all(&tmp);
}

/// `per_record_files` emits one `<Record>.hnl` port/adapter pair per
/// record plus `types.hnl` for typedefs and loose functions — the
/// single-file `<module>.hnl` is not written.
#[test]
fn emits_per_record_layout() {
    let mut module = build_console_module();
    module.per_record_files = true;
    module.records.push(Record {
        name: "Widget".into(),
        fields: vec![],
        size: 0,
        align: 0,
        doc: None,
    });
    module.functions.push(Function {
        name: "widget_find".to_string(),
        doc: None,
        port_name: "WidgetFind".to_string(),
        port_args: vec![],
        arg_descriptors: vec![],
        return_port: "()".to_string(),
        ret_descriptor: "void".to_string(),
        out_struct: None,
        variadic: false,
        c_decl: "Widget.find".to_string(),
    });
    module.typedefs.push(Typedef {
        name: "WidgetInit".into(),
        port: "Handle".into(),
        descriptor: "js_object".into(),
        doc: None,
    });

    let tmp = PathBuf::from(format!(
        "/tmp/sdkgen_emit_per_record_test_{}",
        std::process::id()
    ));
    let port_dir = tmp.join("console");
    let adapter_dir = tmp.join("console-webidl-browser");

    let _ = fs::remove_dir_all(&tmp);
    FileEmitter::new().emit(&module, &port_dir).unwrap();

    let widget_port = fs::read_to_string(port_dir.join("Widget.hnl")).unwrap();
    let types_port = fs::read_to_string(port_dir.join("types.hnl")).unwrap();
    let widget_adapter = fs::read_to_string(adapter_dir.join("Widget.hnl")).unwrap();
    let types_adapter = fs::read_to_string(adapter_dir.join("types.hnl")).unwrap();

    assert!(!port_dir.join("console.hnl").exists());
    assert!(!adapter_dir.join("console.hnl").exists());

    assert!(widget_port.contains("export Widget :="), "{widget_port}");
    assert!(widget_port.contains("export WidgetFind:"), "{widget_port}");
    assert!(types_port.contains("export WidgetInit"), "{types_port}");
    // The loose `console.log` function lands in types.hnl.
    assert!(types_port.contains("export ConsoleLog:"), "{types_port}");

    assert!(
        widget_adapter.contains("from \"../console/Widget\" import { Widget, WidgetFind }"),
        "{widget_adapter}"
    );
    assert!(
        types_adapter.contains("from \"../console/types\" import { ConsoleLog }"),
        "{types_adapter}"
    );
