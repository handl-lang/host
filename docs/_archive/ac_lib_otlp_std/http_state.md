**handl_lib_otlp_std > http_state**

# Module: http_state

## Contents

**Structs**

- [`Event`](#event)
- [`HttpOtlpState`](#httpotlpstate) - Concrete OTLP/HTTP exporter that sends telemetry to an OTLP collector.
- [`LogRecord`](#logrecord)
- [`MetricRecord`](#metricrecord)
- [`Span`](#span)

**Functions**

- [`attribute_array`](#attribute_array)
- [`attribute_value`](#attribute_value)
- [`post_json`](#post_json)
- [`resource_attributes`](#resource_attributes)
- [`scope`](#scope)
- [`send_logs_payload`](#send_logs_payload)
- [`send_metrics_payload`](#send_metrics_payload)
- [`send_trace_payload`](#send_trace_payload)
- [`std_otlp_module`](#std_otlp_module) - Create a preconfigured `otlp` module using [`HttpOtlpState`].
- [`take_ended_spans`](#take_ended_spans)

---

## handl_lib_otlp_std::http_state::Event

*Struct*

**Fields:**
- `name: String`
- `timestamp: u128`
- `attributes: std::collections::HashMap<String, serde_json::Value>`



## handl_lib_otlp_std::http_state::HttpOtlpState

*Struct*

Concrete OTLP/HTTP exporter that sends telemetry to an OTLP collector.

**Fields:**
- `config: std::sync::Mutex<Option<handl_lib_otlp::OtlpConfig>>`
- `spans: std::sync::Mutex<std::collections::HashMap<String, Span>>`
- `logs: std::sync::Mutex<Vec<LogRecord>>`
- `metrics: std::sync::Mutex<Vec<MetricRecord>>`

**Methods:**

- `fn new() -> Self` - Create a new unconfigured exporter.

**Trait Implementations:**

- **OtlpState**
  - `fn configure(self: &Self, config: OtlpConfig) -> OtlpResult<()>`
  - `fn start_span(self: &Self, name: &str, options: StartSpanOptions) -> OtlpResult<String>`
  - `fn end_span(self: &Self, id: &str) -> OtlpResult<()>`
  - `fn set_attribute(self: &Self, id: &str, key: &str, value: Value) -> OtlpResult<()>`
  - `fn add_event(self: &Self, id: &str, name: &str, attributes: HashMap<String, Value>) -> OtlpResult<()>`
  - `fn record_log(self: &Self, level: &str, message: &str, attributes: HashMap<String, Value>) -> OtlpResult<()>`
  - `fn record_metric(self: &Self, name: &str, value: f64, options: MetricOptions) -> OtlpResult<()>`
  - `fn flush(self: &Self) -> OtlpResult<()>`
  - `fn shutdown(self: &Self) -> OtlpResult<()>`
- **Default**
  - `fn default() -> HttpOtlpState`



## handl_lib_otlp_std::http_state::LogRecord

*Struct*

**Fields:**
- `timestamp: u128`
- `level: String`
- `message: String`
- `attributes: std::collections::HashMap<String, serde_json::Value>`



## handl_lib_otlp_std::http_state::MetricRecord

*Struct*

**Fields:**
- `timestamp: u128`
- `name: String`
- `value: f64`
- `kind: handl_lib_otlp::MetricKind`
- `attributes: std::collections::HashMap<String, serde_json::Value>`



## handl_lib_otlp_std::http_state::Span

*Struct*

**Fields:**
- `id: String`
- `trace_id: String`
- `parent_id: Option<String>`
- `name: String`
- `start_time: u128`
- `end_time: Option<u128>`
- `attributes: std::collections::HashMap<String, serde_json::Value>`
- `events: Vec<Event>`



## handl_lib_otlp_std::http_state::attribute_array

*Function*

```rust
fn attribute_array(attrs: &std::collections::HashMap<String, serde_json::Value>) -> serde_json::Value
```



## handl_lib_otlp_std::http_state::attribute_value

*Function*

```rust
fn attribute_value(value: &serde_json::Value) -> serde_json::Value
```



## handl_lib_otlp_std::http_state::post_json

*Function*

```rust
fn post_json(config: &handl_lib_otlp::OtlpConfig, url: &str, payload: &serde_json::Value) -> handl_lib_otlp::OtlpResult<()>
```



## handl_lib_otlp_std::http_state::resource_attributes

*Function*

```rust
fn resource_attributes(config: &handl_lib_otlp::OtlpConfig) -> serde_json::Value
```



## handl_lib_otlp_std::http_state::scope

*Function*

```rust
fn scope() -> serde_json::Value
```



## handl_lib_otlp_std::http_state::send_logs_payload

*Function*

```rust
fn send_logs_payload(config: &handl_lib_otlp::OtlpConfig, logs: &[LogRecord]) -> handl_lib_otlp::OtlpResult<()>
```



## handl_lib_otlp_std::http_state::send_metrics_payload

*Function*

```rust
fn send_metrics_payload(config: &handl_lib_otlp::OtlpConfig, metrics: &[MetricRecord]) -> handl_lib_otlp::OtlpResult<()>
```



## handl_lib_otlp_std::http_state::send_trace_payload

*Function*

```rust
fn send_trace_payload(config: &handl_lib_otlp::OtlpConfig, spans: &[Span]) -> handl_lib_otlp::OtlpResult<()>
```



## handl_lib_otlp_std::http_state::std_otlp_module

*Function*

Create a preconfigured `otlp` module using [`HttpOtlpState`].

```rust
fn std_otlp_module() -> handl_lib_otlp::OtlpModule
```



## handl_lib_otlp_std::http_state::take_ended_spans

*Function*

```rust
fn take_ended_spans(spans: &std::sync::Mutex<std::collections::HashMap<String, Span>>) -> Vec<Span>
```



