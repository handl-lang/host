//! Integration tests for the `otlp` package row — the conn-as-value `.hnl`
//! contract over the `otlp_c` native-ops backend.
//!
//! BACKLOG ("`otlp.configure` 404s"): the contract's `otlp_c.*` qualified
//! calls must resolve through the module table — `from "otlp_c" import * as
//! otlp_c` binds each backend op as the string callee `"otlp_c.<op>"`, and
//! applying it dispatches through the module's global
//! [`TableModuleLookup`], which resolves `otlp_c` through the resolver the
//! module was built with. These tests pin the whole chain:
//! `module_with("otlp", …)` exposes `configure` (the empty-record failure
//! mode would 404 here), and the call reaches the backend — a
//! [`StubOtlpState`] (`lib-otlp-c`, in-memory, no network) injected as the
//! resolver's `otlp_c` row, exactly the seam production uses to inject the
//! HTTP exporter (`lib-otlp-std`).
//!
//! [`TableModuleLookup`]: handl_plugin_lib_stdlib::TableModuleLookup
//! [`StubOtlpState`]: handl_plugin_lib_otlp_c::StubOtlpState

use std::sync::Arc;

use handl_platform::{EvalContext, Module};
use handl_plugin_lib_handl::ModuleResolver;
use handl_plugin_lib_otlp_c::{OtlpEvent, OtlpModule, OtlpState, StubOtlpState};
use handl_plugin_lib_stdlib::module_with;
use serde_json::json;

/// Install the canonical default language once per test binary (see
/// `json.rs` — the `HandlModule` bridge runtimes resolve it lazily).
fn ensure_lang() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(
            handl_kit_data_prelude::language_from_yaml().expect("canonical language assembles"),
        );
    }
}

/// Resolver that answers `otlp_c` with an inert stub-backed backend and
/// defers everything else to the package table (the lookup's
/// `module_ref` fallback for table names).
///
/// The stub must shadow the table's own `otlp_c` row (the real
/// `HttpOtlpState` exporter): [`TableModuleLookup`] consults this resolver
/// first, so every `otlp_c.*` callee the contract applies lands in the
/// stub — no network in tests.
///
/// [`TableModuleLookup`]: handl_plugin_lib_stdlib::TableModuleLookup
struct StubBackendResolver {
    state: Arc<StubOtlpState>,
}

impl ModuleResolver for StubBackendResolver {
    fn resolve(&self, name: &str) -> Option<Arc<dyn Module + Send + Sync>> {
        if name == "otlp_c" {
            let state: Arc<dyn OtlpState> = self.state.clone();
            return Some(Arc::new(OtlpModule::new(state)));
        }
        None
    }
}

/// `module_with("otlp", …)` with the stub `otlp_c` backend.
fn otlp_with_stub(state: Arc<StubOtlpState>) -> handl_plugin_lib_handl::HandlModule {
    ensure_lang();
    module_with("otlp", Arc::new(StubBackendResolver { state }))
        .expect("otlp is in the package table")
}

fn call(module: &dyn Module, name: &str, args: &[serde_json::Value]) -> serde_json::Value {
    module
        .function(name)
        .unwrap_or_else(|| panic!("otlp.{name} bound"))
        .call(args, &mut EvalContext::new())
        .unwrap_or_else(|e| panic!("otlp.{name} calls through: {e}"))
}

/// The contract record exposes every declared field — `configure` above all
/// (the 404 symptom was the record evaluating empty, so `function` lookups
/// missed).
#[test]
fn otlp_contract_row_exposes_configure() {
    let module = otlp_with_stub(Arc::new(StubOtlpState::new()));
    assert_eq!(module.name(), "otlp");

    for field in [
        "configure",
        "start_span",
        "end_span",
        "set_attribute",
        "add_event",
        "record_log",
        "record_metric",
        "flush",
        "shutdown",
    ] {
        assert!(
            module.function(field).is_some(),
            "otlp.{field} must be bound by the contract record"
        );
    }
}

/// `otlp.configure(url)` evaluates, returns the conn-as-value record, and
/// reaches the `otlp_c.configure` backend op with the endpoint config.
#[test]
fn otlp_configure_calls_through_to_backend() {
    let state = Arc::new(StubOtlpState::new());
    let module = otlp_with_stub(state.clone());

    let conn = call(&module, "configure", &[json!("http://localhost:4318")]);

    // Conn-as-value: the configure arrow returns `{endpoint: url}`.
    assert_eq!(
        conn.get("endpoint").and_then(|v| v.as_str()),
        Some("http://localhost:4318"),
        "configure returns the conn record: {conn:?}"
    );

    // Call-through: the backend op ran with the parsed config.
    let events = state.events();
    assert_eq!(events.len(), 1, "exactly one backend op ran");
    assert!(
        matches!(
            &events[0],
            OtlpEvent::Configured(cfg) if cfg.endpoint == "http://localhost:4318"
        ),
        "otlp_c.configure received the endpoint config: {:?}",
        events[0]
    );
}

/// A span opened on the returned conn reaches `otlp_c.start_span` and
/// yields the backend-allocated span id (stateful backend ops — the conn
/// is a token; the state carries it).
#[test]
fn otlp_start_span_calls_through_and_returns_span_id() {
    let state = Arc::new(StubOtlpState::new());
    let module = otlp_with_stub(state.clone());

    let conn = call(&module, "configure", &[json!("http://localhost:4318")]);
    let span = call(&module, "start_span", &[conn, json!("test.op")]);

    let id = span
        .as_str()
        .unwrap_or_else(|| panic!("start_span returns the backend span id: {span:?}"))
        .to_string();
    assert!(!id.is_empty(), "span id is non-empty");

    let events = state.events();
    assert_eq!(
        events.len(),
        2,
        "configure + start_span reached the backend"
    );
    assert!(matches!(&events[0], OtlpEvent::Configured(_)));
    assert!(
        matches!(&events[1], OtlpEvent::SpanStarted(s) if s.name == "test.op"),
        "otlp_c.start_span received the span name: {:?}",
        events[1]
    );

    // Ending the span routes through too.
    let _ = call(&module, "end_span", &[json!(id)]);
    assert!(matches!(
        state.events().last(),
        Some(OtlpEvent::SpanEnded { .. })
    ));
}

/// The remaining contract fields dispatch through the backend ops without
/// touching the network (stub backend records every op).
#[test]
fn otlp_remaining_ops_dispatch_through_backend() {
    let state = Arc::new(StubOtlpState::new());
    let module = otlp_with_stub(state.clone());

    let conn = call(&module, "configure", &[json!("http://localhost:4318")]);
    let span = call(&module, "start_span", &[conn.clone(), json!("op")]);

    let _ = call(
        &module,
        "set_attribute",
        &[span.clone(), json!({"key": "answer", "value": 42})],
    );
    let _ = call(
        &module,
        "add_event",
        &[span, json!({"name": "evt", "attributes": {}})],
    );
    let _ = call(
        &module,
        "record_log",
        &[
            conn.clone(),
            json!({"level": "INFO", "message": "hello", "attributes": {}}),
        ],
    );
    let _ = call(
        &module,
        "record_metric",
        &[
            conn.clone(),
            json!({"name": "requests", "value": 1, "kind": "counter"}),
        ],
    );
    let _ = call(&module, "flush", &[conn.clone()]);
    let _ = call(&module, "shutdown", &[conn]);

    let events = state.events();
    let kinds = [
        matches!(events[0], OtlpEvent::Configured(_)),
        matches!(events[1], OtlpEvent::SpanStarted(_)),
        matches!(events[2], OtlpEvent::SpanAttribute { .. }),
        matches!(events[3], OtlpEvent::SpanEvent { .. }),
        matches!(events[4], OtlpEvent::LogRecorded { .. }),
        matches!(events[5], OtlpEvent::MetricRecorded { .. }),
        matches!(events[6], OtlpEvent::Flushed),
        matches!(events[7], OtlpEvent::Shutdown),
    ];
    assert!(
        kinds.iter().all(|k| *k),
        "every contract field dispatched to the backend: {:?}",
        events
    );
}
