//! `OtlpModule` — the host [`Module`] adapter for the `otlp_c.*` ops.
//!
//! Relocated from `handl-plugin-lib-otlp-c` in the `handl-ops` migration:
//! the lib repo now exposes only the kernel [`handl_ops::OpRegistry`]
//! ([`handl_plugin_lib_otlp_c::otlp_op_registry`]); this wrapper builds the
//! host [`HandlModule`] and imports the kernel ops via
//! [`OpRegistry::import_kernel`].

use std::sync::Arc;

use handl_platform::Module;
use handl_plugin_lib_handl::{HandlModule, OpRegistry};

use handl_plugin_lib_otlp_c::{OTLP_C_HNL, OTLP_OPS, OtlpState, otlp_op_registry};

/// Module adapter for [`OtlpState`] — exposes `otlp_c.*` functions.
///
/// | Function | Args | Description |
/// |----------|------|-------------|
/// | `otlp_c.configure(config)` | `(config)` | Set endpoint, service name, headers and resource attributes. |
/// | `otlp_c.start_span(name, options?)` | `(name, options?)` | Start a span and return its id. |
/// | `otlp_c.end_span(id)` | `(id)` | End a span. |
/// | `otlp_c.set_attribute(id, key, value)` | `(id, key, value)` | Set a span attribute. |
/// | `otlp_c.add_event(id, name, attrs?)` | `(id, name, attrs?)` | Add a named event to a span. |
/// | `otlp_c.record_log(level, message, attrs?)` | `(level, message, attrs?)` | Record a log entry. |
/// | `otlp_c.record_metric(name, value, options?)` | `(name, value, options?)` | Record a numeric metric. |
/// | `otlp_c.flush()` | `()` | Export all buffered telemetry. |
/// | `otlp_c.shutdown()` | `()` | End all spans, flush and release the exporter. |
///
/// The interface is declared in `otlp.hnl` via `op`/`handl` declarations
/// and loaded via the generic [`HandlModule`]; this struct delegates
/// [`Module`] lookups to the [`HandlModule`] built from the state.
pub struct OtlpModule {
    module: HandlModule,
}

impl OtlpModule {
    /// Create a new module backed by the given state.
    pub fn new(state: Arc<dyn OtlpState>) -> Self {
        let mut registry = OpRegistry::new();
        registry.import_kernel(&otlp_op_registry(state));
        Self {
            module: HandlModule::new("otlp_c", OTLP_C_HNL, &[])
                .with_ops(OTLP_OPS, Arc::new(registry)),
        }
    }
}

impl Module for OtlpModule {
    fn name(&self) -> &str {
        "otlp_c"
    }

    fn function(&self, name: &str) -> Option<&dyn handl_platform::Function> {
        self.module.function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn handl_platform::Function)> {
        self.module.functions()
    }
}

#[cfg(test)]
mod tests {
    use handl_platform::EvalContext;
    use handl_platform::Module;
    use serde_json::Value;
    use std::sync::Arc;

    use handl_plugin_lib_otlp_c::{OtlpEvent, StubOtlpState};

    use super::OtlpModule;

    fn s(v: &str) -> serde_json::Value {
        serde_json::Value::from(v)
    }

    fn obj(v: Value) -> serde_json::Value {
        serde_json::Value::from(v)
    }

    fn call(module: &dyn Module, name: &str, args: &[serde_json::Value]) -> serde_json::Value {
        let func = module.function(name).expect("function exists");
        let mut ctx = EvalContext::new();
        func.call(args, &mut ctx).expect("call succeeds")
    }

    #[test]
    fn module_name() {
        let module = OtlpModule::new(Arc::new(StubOtlpState::new()));
        assert_eq!(module.name(), "otlp_c");
    }

    #[test]
    fn configure_is_recorded() {
        let state = Arc::new(StubOtlpState::new());
        let module = OtlpModule::new(state.clone());
        call(
            &module,
            "configure",
            &[obj(serde_json::json!({
                "endpoint": "http://localhost:4318",
                "service_name": "test"
            }))],
        );
        assert!(matches!(state.events()[0], OtlpEvent::Configured(_)));
    }

    #[test]
    fn start_span_returns_id() {
        let state = Arc::new(StubOtlpState::new());
        let module = OtlpModule::new(state.clone());
        let id = call(&module, "start_span", &[s("op")]);
        assert!(id.as_str().is_some());
        assert!(matches!(state.events()[0], OtlpEvent::SpanStarted(_)));
    }

    #[test]
    fn start_span_with_parent() {
        let state = Arc::new(StubOtlpState::new());
        let module = OtlpModule::new(state.clone());
        let parent = call(
            &module,
            "start_span",
            &[
                s("parent"),
                obj(serde_json::json!({"attributes": {"a": 1}})),
            ],
        );
        let child = call(
            &module,
            "start_span",
            &[
                s("child"),
                obj(serde_json::json!({"parent": parent.as_str().unwrap()})),
            ],
        );
        assert!(child.as_str().is_some());
        assert_eq!(state.events().len(), 2);
    }

    #[test]
    fn end_span_is_recorded() {
        let state = Arc::new(StubOtlpState::new());
        let module = OtlpModule::new(state.clone());
        let id = call(&module, "start_span", &[s("op")]);
        call(&module, "end_span", &[id]);
        assert!(matches!(state.events()[1], OtlpEvent::SpanEnded { .. }));
    }

    #[test]
    fn set_attribute_is_recorded() {
        let state = Arc::new(StubOtlpState::new());
        let module = OtlpModule::new(state.clone());
        let id = call(&module, "start_span", &[s("op")]);
        call(
            &module,
            "set_attribute",
            &[id.clone(), s("key"), serde_json::Value::from(42)],
        );
        assert!(matches!(state.events()[1], OtlpEvent::SpanAttribute { .. }));
    }

    #[test]
    fn add_event_is_recorded() {
        let state = Arc::new(StubOtlpState::new());
        let module = OtlpModule::new(state.clone());
        let id = call(&module, "start_span", &[s("op")]);
        call(
            &module,
            "add_event",
            &[
                id.clone(),
                s("event"),
                obj(serde_json::json!({"attributes": {"x": true}})),
            ],
        );
        assert!(matches!(state.events()[1], OtlpEvent::SpanEvent { .. }));
    }

    #[test]
    fn record_log_is_recorded() {
        let state = Arc::new(StubOtlpState::new());
        let module = OtlpModule::new(state.clone());
        call(
            &module,
            "record_log",
            &[s("INFO"), s("hello"), obj(serde_json::json!({"attr": "v"}))],
        );
        assert!(matches!(state.events()[0], OtlpEvent::LogRecorded { .. }));
    }

    #[test]
    fn record_metric_is_recorded() {
        let state = Arc::new(StubOtlpState::new());
        let module = OtlpModule::new(state.clone());
        call(
            &module,
            "record_metric",
            &[
                s("requests"),
                serde_json::Value::from(1.0),
                obj(serde_json::json!({"kind": "counter"})),
            ],
        );
        assert!(matches!(
            state.events()[0],
            OtlpEvent::MetricRecorded { .. }
        ));
    }

    #[test]
    fn flush_and_shutdown_are_recorded() {
        let state = Arc::new(StubOtlpState::new());
        let module = OtlpModule::new(state.clone());
        call(&module, "flush", &[]);
        call(&module, "shutdown", &[]);
        assert!(matches!(state.events()[0], OtlpEvent::Flushed));
        assert!(matches!(state.events()[1], OtlpEvent::Shutdown));
    }
}
