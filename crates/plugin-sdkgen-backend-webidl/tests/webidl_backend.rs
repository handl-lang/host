use handl_plugin_sdkgen::SdkgenBackend;
use handl_plugin_sdkgen_backend_webidl::WebIdlBackend;
use std::path::Path;

#[test]
fn parses_console_webidl() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest.join("tests/fixtures/console.webidl");
    let backend = WebIdlBackend::new(None);
    let module = backend.parse_header(&path).expect("parse fixture");

    assert!(module.diagnostics.is_empty(), "{:?}", module.diagnostics);

    assert_eq!(module.client_package, "js");
    assert_eq!(module.backend_id, "webidl");
    assert_eq!(module.target, "browser");

    let event = module
        .records
        .iter()
        .find(|r| r.name == "Event")
        .expect("Event record");
    assert_eq!(event.fields.len(), 2);
    assert_eq!(
        event.fields.iter().map(|f| &f.name).collect::<Vec<_>>(),
        vec!["type", "bubbles"]
    );
    assert_eq!(event.fields[0].descriptor, "js_string");
    assert!(event.fields[0].is_const);
    assert_eq!(event.fields[1].descriptor, "bool");

    let event_init = module
        .records
        .iter()
        .find(|r| r.name == "EventInit")
        .expect("EventInit record");
    assert_eq!(event_init.fields.len(), 2);
    assert_eq!(event_init.fields[0].descriptor, "bool");
    assert_eq!(event_init.fields[1].descriptor, "bool");

    // Partial interface merges in the isTrusted operation.
    let stop = module
        .functions
        .iter()
        .find(|f| f.name == "event_stop_propagation")
        .expect("stopPropagation");
    assert_eq!(stop.port_name, "EventStopPropagation");
    assert_eq!(stop.return_port, "()");
    assert_eq!(stop.ret_descriptor, "void");

    let is_trusted = module
        .functions
        .iter()
        .find(|f| f.name == "event_is_trusted")
        .expect("isTrusted");
    assert_eq!(is_trusted.port_name, "EventIsTrusted");
    assert_eq!(is_trusted.return_port, "Bool");
    assert_eq!(is_trusted.ret_descriptor, "bool");

    let console_log = module
        .functions
        .iter()
        .find(|f| f.name == "console_log")
        .expect("console.log");
    assert_eq!(console_log.port_name, "ConsoleLog");
    assert_eq!(console_log.arg_descriptors, &["js_string"]);
    assert_eq!(console_log.ret_descriptor, "void");

    let console_warn = module
        .functions
        .iter()
        .find(|f| f.name == "console_warn")
        .expect("console.warn");
    assert!(console_warn.variadic);
    assert_eq!(console_warn.arg_descriptors, &["js_object"]);

    let callback = module
        .functions
        .iter()
        .find(|f| f.name == "frame_request_callback")
        .expect("FrameRequestCallback");
    assert_eq!(callback.port_name, "FrameRequestCallback");
    assert_eq!(callback.ret_descriptor, "void");

    let mixed = module
        .typedefs
        .iter()
        .find(|t| t.name == "MixedBytes")
        .expect("MixedBytes");
    assert_eq!(mixed.descriptor, "js_object");

    let open = module
        .enum_constants
        .iter()
        .find(|c| c.name == "ShadowRootMode_Open")
        .expect("Open");
    assert_eq!(open.value, 0);
    let closed = module
        .enum_constants
        .iter()
        .find(|c| c.name == "ShadowRootMode_Closed")
        .expect("Closed");
    assert_eq!(closed.value, 1);
}
