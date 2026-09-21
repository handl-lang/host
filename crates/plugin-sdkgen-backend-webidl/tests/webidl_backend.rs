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

    // In-source docs: the interface comment becomes the record doc and
    // the member comment lands on its field.
    assert!(event.doc.as_deref().unwrap().contains("basis for the DOM"));
    assert!(
        event.fields[1]
            .doc
            .as_deref()
            .unwrap()
            .contains("bubbles through")
    );

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

/// `static` ops, `stringifier attribute`, bare `stringifier;`, and
/// `iterable<K,V>` members must parse without diagnostics — statics map to
/// functions, the stringifier attribute to a readonly field, and the
/// container/stringifier markers drop cleanly.
#[test]
fn parses_extended_member_forms() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest.join("tests/fixtures/members.webidl");
    let backend = WebIdlBackend::new(None);
    let module = backend.parse_header(&path).expect("parse fixture");

    assert!(module.diagnostics.is_empty(), "{:?}", module.diagnostics);

    let widget = module
        .records
        .iter()
        .find(|r| r.name == "Widget")
        .expect("Widget record");
    assert_eq!(
        widget.fields.iter().map(|f| &f.name).collect::<Vec<_>>(),
        vec!["href", "origin"]
    );

    for port in ["WidgetFind", "WidgetCanFind"] {
        assert!(
            module.functions.iter().any(|f| f.port_name == port),
            "missing {port}"
        );
    }
    let find = module
        .functions
        .iter()
        .find(|f| f.port_name == "WidgetFind")
        .unwrap();
    assert_eq!(find.arg_descriptors, &["js_string"]);
}

/// `A includes B;` grafts the mixin's members onto the target interface:
/// `Request`/`Response` gain `Body`'s ops as their own ports while `Body`
/// keeps its own. Missing targets/mixins surface as diagnostics.
#[test]
fn includes_grafts_mixin_members() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest.join("tests/fixtures/includes.webidl");
    let backend = WebIdlBackend::new(None);
    let module = backend.parse_header(&path).expect("parse fixture");

    for port in [
        "BodyText",
        "RequestText",
        "RequestBlob",
        "ResponseText",
        "ResponseBlob",
    ] {
        assert!(
            module.functions.iter().any(|f| f.port_name == port),
            "missing {port}"
        );
    }
    let req = module
        .records
        .iter()
        .find(|r| r.name == "Request")
        .expect("Request record");
    assert!(
        req.fields.iter().any(|f| f.name == "bodyUsed"),
        "grafted attribute missing"
    );
    assert_eq!(
        module.diagnostics,
        vec![
            "includes: Ghost is not an interface in this spec",
            "includes: mixin MissingMixin not found (on Request)",
        ]
    );
}

/// A supplementary spec (`add_include_spec`) supplies mixin definitions
/// the primary spec references but does not declare — the graft resolves
/// and the diagnostic clears. Supplementary interfaces are not emitted.
#[test]
fn includes_resolve_across_specs() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest.join("tests/fixtures/includes.webidl");
    let extra = manifest.join("tests/fixtures/includes-extra.webidl");
    let mut backend = WebIdlBackend::new(None);
    backend.add_include_spec(&extra);
    let module = backend.parse_header(&path).expect("parse fixture");

    let req = module
        .records
        .iter()
        .find(|r| r.name == "Request")
        .expect("Request record");
    assert!(
        req.fields.iter().any(|f| f.name == "extra"),
        "cross-spec grafted attribute missing"
    );
    assert!(
        !module.records.iter().any(|r| r.name == "SupplementaryOnly"),
        "supplementary spec emitted a record"
    );
    assert_eq!(
        module.diagnostics,
        vec!["includes: Ghost is not an interface in this spec"]
    );
}

/// An `includes` whose *target* is foreign (storage.idl's
/// `Navigator includes NavigatorStorage`, where `Navigator` is html.idl's)
/// grafts onto the supplementary interface and emits the augmented record
/// in the primary package.
#[test]
fn includes_graft_onto_foreign_interface() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest.join("tests/fixtures/includes-foreign.webidl");
    let extra = manifest.join("tests/fixtures/includes-extra.webidl");
    let mut backend = WebIdlBackend::new(None);
    backend.add_include_spec(&extra);
    let module = backend.parse_header(&path).expect("parse fixture");

    let foreign = module
        .records
        .iter()
        .find(|r| r.name == "Foreign")
        .expect("Foreign record emitted in primary package");
    for field in ["ownMember", "bodyUsed"] {
        assert!(
            foreign.fields.iter().any(|f| f.name == field),
            "grafted/own field {field} missing"
        );
    }
    assert!(
        module.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        module.diagnostics
    );
}

/// Enum constants are gathered through a `HashMap` merge; without a sort the
/// emitted order varies per run (fresh `RandomState`). Parsing twice in one
/// process must yield identical output.
#[test]
fn enum_constants_are_deterministic() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest.join("tests/fixtures/enums.webidl");
    let backend = WebIdlBackend::new(None);

    let names = |m: &handl_plugin_sdkgen::SdkgenModule| {
        m.enum_constants
            .iter()
            .map(|c| c.name.clone())
            .collect::<Vec<_>>()
    };
    let first = backend.parse_header(&path).expect("parse fixture");
    for _ in 0..16 {
        let again = backend.parse_header(&path).expect("parse fixture");
        assert_eq!(names(&first), names(&again));
    }
    assert_eq!(
        names(&first),
        vec![
            "AlphaMode_X",
            "AlphaMode_Y",
            "MidMode_M",
            "MidMode_N",
            "ZetaMode_A",
            "ZetaMode_B"
        ]
    );
}
