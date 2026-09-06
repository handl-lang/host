use std::collections::HashMap;
use std::sync::Arc;

use handl_platform::Module;
use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::{HandlModule, OpRegistry};
use serde_json::Value;

use crate::state::{MetricKind, MetricOptions, OtlpConfig, OtlpState, StartSpanOptions};

/// The handl source declaring the otlp `op` interface — the single source
/// of truth for the OTLP API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const OTLP_HNL: &str = include_str!("../../../handl-packages/otlp/otlp.hnl");

/// Module adapter for [`OtlpState`] — exposes `otlp.*` functions.
///
/// | Function | Args | Description |
/// |----------|------|-------------|
/// | `otlp.configure(config)` | `(config)` | Set endpoint, service name, headers and resource attributes. |
/// | `otlp.start_span(name, options?)` | `(name, options?)` | Start a span and return its id. |
/// | `otlp.end_span(id)` | `(id)` | End a span. |
/// | `otlp.set_attribute(id, key, value)` | `(id, key, value)` | Set a span attribute. |
/// | `otlp.add_event(id, name, attrs?)` | `(id, name, attrs?)` | Add a named event to a span. |
/// | `otlp.record_log(level, message, attrs?)` | `(level, message, attrs?)` | Record a log entry. |
/// | `otlp.record_metric(name, value, options?)` | `(name, value, options?)` | Record a numeric metric. |
/// | `otlp.flush()` | `()` | Export all buffered telemetry. |
/// | `otlp.shutdown()` | `()` | End all spans, flush and release the exporter. |
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
        register_otlp_ops(&mut registry, state);
        Self {
            module: HandlModule::new("otlp", OTLP_HNL, &[]).with_ops(OTLP_OPS, Arc::new(registry)),
        }
    }
}

impl Module for OtlpModule {
    fn name(&self) -> &str {
        "otlp"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        self.module.function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        self.module.functions()
    }
}

fn require_string_at(args: &[serde_json::Value], idx: usize) -> Result<&str, EvalError> {
    args.get(idx)
        .ok_or(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            idx + 1,
            args.len(),
        )))?
        .as_str()
        .ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "string", "non-string",
        )))
}

fn require_number_at(args: &[serde_json::Value], idx: usize) -> Result<f64, EvalError> {
    args.get(idx)
        .ok_or(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            idx + 1,
            args.len(),
        )))?
        .as_f64()
        .ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number",
        )))
}

fn require_object_at(
    args: &[serde_json::Value],
    idx: usize,
) -> Result<&serde_json::Map<String, Value>, EvalError> {
    args.get(idx)
        .ok_or(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            idx + 1,
            args.len(),
        )))?
        .as_object()
        .ok_or(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "object", "non-object",
        )))
}

fn optional_object_at(
    args: &[serde_json::Value],
    idx: usize,
) -> Option<&serde_json::Map<String, Value>> {
    args.get(idx).and_then(|v| v.as_object())
}

fn get_string<'a>(obj: &'a serde_json::Map<String, Value>, key: &str) -> Option<&'a str> {
    obj.get(key).and_then(|v| v.as_str())
}

fn value_map_to_hashmap(map: &serde_json::Map<String, Value>) -> HashMap<String, Value> {
    map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

fn parse_headers(
    map: &serde_json::Map<String, Value>,
) -> Result<HashMap<String, String>, EvalError> {
    map.iter()
        .map(|(k, v)| {
            v.as_str()
                .map(|s| (k.clone(), s.to_string()))
                .ok_or(EvalError::TypeMismatch(format!(
                    "expected {}, got {}",
                    "string", "non-string",
                )))
        })
        .collect()
}

fn parse_config(args: &[serde_json::Value]) -> Result<OtlpConfig, EvalError> {
    let obj = require_object_at(args, 0)?;
    let endpoint = get_string(obj, "endpoint").unwrap_or("").to_string();
    let service_name = get_string(obj, "service_name")
        .unwrap_or("handl")
        .to_string();
    let resource_attributes = obj
        .get("resource_attributes")
        .and_then(|v| v.as_object())
        .map(value_map_to_hashmap)
        .unwrap_or_default();
    let headers = obj
        .get("headers")
        .and_then(|v| v.as_object())
        .map(parse_headers)
        .transpose()?
        .unwrap_or_default();
    Ok(OtlpConfig {
        endpoint,
        service_name,
        resource_attributes,
        headers,
    })
}

fn parse_attributes(obj: &serde_json::Map<String, Value>, key: &str) -> HashMap<String, Value> {
    obj.get(key)
        .and_then(|v| v.as_object())
        .map(value_map_to_hashmap)
        .unwrap_or_default()
}

struct ConfigureFn {
    state: Arc<dyn OtlpState>,
}

impl Function for ConfigureFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let config = parse_config(args)?;
        self.state.configure(config).map_err(EvalError::from)?;
        Ok(serde_json::Value::from(Value::Null))
    }
}

struct StartSpanFn {
    state: Arc<dyn OtlpState>,
}

impl Function for StartSpanFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let name = require_string_at(args, 0)?;
        let mut options = StartSpanOptions::default();
        if let Some(obj) = optional_object_at(args, 1) {
            options.parent = get_string(obj, "parent").map(|s| s.to_string());
            options.attributes = parse_attributes(obj, "attributes");
        }
        let id = self
            .state
            .start_span(name, options)
            .map_err(EvalError::from)?;
        Ok(serde_json::Value::from(id))
    }
}

struct EndSpanFn {
    state: Arc<dyn OtlpState>,
}

impl Function for EndSpanFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let id = require_string_at(args, 0)?;
        self.state.end_span(id).map_err(EvalError::from)?;
        Ok(serde_json::Value::from(Value::Null))
    }
}

struct SetAttributeFn {
    state: Arc<dyn OtlpState>,
}

impl Function for SetAttributeFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let id = require_string_at(args, 0)?;
        let key = require_string_at(args, 1)?;
        let value = args
            .get(2)
            .ok_or(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                3,
                args.len(),
            )))?
            .clone();
        self.state
            .set_attribute(id, key, value)
            .map_err(EvalError::from)?;
        Ok(serde_json::Value::from(Value::Null))
    }
}

struct AddEventFn {
    state: Arc<dyn OtlpState>,
}

impl Function for AddEventFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let id = require_string_at(args, 0)?;
        let name = require_string_at(args, 1)?;
        let attributes = args
            .get(2)
            .and_then(|v| v.as_object())
            .map(value_map_to_hashmap)
            .unwrap_or_default();
        self.state
            .add_event(id, name, attributes)
            .map_err(EvalError::from)?;
        Ok(serde_json::Value::from(Value::Null))
    }
}

struct RecordLogFn {
    state: Arc<dyn OtlpState>,
}

impl Function for RecordLogFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let level = require_string_at(args, 0)?;
        let message = require_string_at(args, 1)?;
        let attributes = args
            .get(2)
            .and_then(|v| v.as_object())
            .map(value_map_to_hashmap)
            .unwrap_or_default();
        self.state
            .record_log(level, message, attributes)
            .map_err(EvalError::from)?;
        Ok(serde_json::Value::from(Value::Null))
    }
}

struct RecordMetricFn {
    state: Arc<dyn OtlpState>,
}

impl Function for RecordMetricFn {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let name = require_string_at(args, 0)?;
        let value = require_number_at(args, 1)?;
        let mut options = MetricOptions::default();
        if let Some(obj) = optional_object_at(args, 2) {
            options.kind = get_string(obj, "kind")
                .and_then(MetricKind::parse)
                .unwrap_or(MetricKind::Counter);
            options.attributes = parse_attributes(obj, "attributes");
        }
        self.state
            .record_metric(name, value, options)
            .map_err(EvalError::from)?;
        Ok(serde_json::Value::from(Value::Null))
    }
}

struct FlushFn {
    state: Arc<dyn OtlpState>,
}

impl Function for FlushFn {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        self.state.flush().map_err(EvalError::from)?;
        Ok(serde_json::Value::from(Value::Null))
    }
}

struct ShutdownFn {
    state: Arc<dyn OtlpState>,
}

impl Function for ShutdownFn {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        self.state.shutdown().map_err(EvalError::from)?;
        Ok(serde_json::Value::from(Value::Null))
    }
}

// ---------------------------------------------------------------------------
// Op Registry (ADR-0077 Phase 4)
// ---------------------------------------------------------------------------

/// The op names declared in `otlp.hnl` — the OTLP interface.
pub const OTLP_OPS: &[&str] = &[
    "otlp.configure",
    "otlp.start_span",
    "otlp.end_span",
    "otlp.set_attribute",
    "otlp.add_event",
    "otlp.record_log",
    "otlp.record_metric",
    "otlp.flush",
    "otlp.shutdown",
];

/// Register the `otlp` ops into the given [`OpRegistry`], dispatching to
/// the given [`OtlpState`] implementation.
pub fn register_otlp_ops(registry: &mut OpRegistry, state: Arc<dyn OtlpState>) {
    registry.register("otlp.configure", Arc::new(ConfigureFn { state: state.clone() }));
    registry.register("otlp.start_span", Arc::new(StartSpanFn { state: state.clone() }));
    registry.register("otlp.end_span", Arc::new(EndSpanFn { state: state.clone() }));
    registry.register("otlp.set_attribute", Arc::new(SetAttributeFn { state: state.clone() }));
    registry.register("otlp.add_event", Arc::new(AddEventFn { state: state.clone() }));
    registry.register("otlp.record_log", Arc::new(RecordLogFn { state: state.clone() }));
    registry.register("otlp.record_metric", Arc::new(RecordMetricFn { state: state.clone() }));
    registry.register("otlp.flush", Arc::new(FlushFn { state: state.clone() }));
    registry.register("otlp.shutdown", Arc::new(ShutdownFn { state }));
}

#[cfg(test)]
mod tests;
