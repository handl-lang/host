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
    let adapter =
        fs::read_to_string(tmp.join("console-webidl-unknown-browser/console.hnl")).unwrap();

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
        port.contains("Result < (), JsError >"),
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
