**handl_lib_otlp > stub**

# Module: stub

## Contents

**Structs**

- [`RecordedEvent`](#recordedevent) - A span event that was recorded by the stub.
- [`RecordedSpan`](#recordedspan) - A span that was recorded by the stub.
- [`StubOtlpState`](#stubotlpstate) - In-memory OTLP state for testing.

**Enums**

- [`OtlpEvent`](#otlpevent) - Events recorded by [`StubOtlpState`].

---

## handl_lib_otlp::stub::OtlpEvent

*Enum*

Events recorded by [`StubOtlpState`].

**Variants:**
- `Configured(crate::state::OtlpConfig)` - `configure` was called.
- `SpanStarted(RecordedSpan)` - A span was started.
- `SpanEnded{ id: String }` - A span was ended.
- `SpanAttribute{ id: String, key: String, value: serde_json::Value }` - A span attribute was set.
- `SpanEvent{ id: String, name: String, attributes: std::collections::HashMap<String, serde_json::Value> }` - A span event was added.
- `LogRecorded{ level: String, message: String, attributes: std::collections::HashMap<String, serde_json::Value> }` - A log was recorded.
- `MetricRecorded{ name: String, value: f64, options: crate::state::MetricOptions }` - A metric was recorded.
- `Flushed` - `flush` was called.
- `Shutdown` - `shutdown` was called.

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> OtlpEvent`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_lib_otlp::stub::RecordedEvent

*Struct*

A span event that was recorded by the stub.

**Fields:**
- `name: String` - Event name.
- `attributes: std::collections::HashMap<String, serde_json::Value>` - Event attributes.

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> RecordedEvent`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_lib_otlp::stub::RecordedSpan

*Struct*

A span that was recorded by the stub.

**Fields:**
- `id: String` - Span id.
- `trace_id: String` - Trace id.
- `parent_id: Option<String>` - Parent span id, if any.
- `name: String` - Span name.
- `attributes: std::collections::HashMap<String, serde_json::Value>` - Span attributes.
- `events: Vec<RecordedEvent>` - Events recorded on the span.

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> RecordedSpan`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_lib_otlp::stub::StubOtlpState

*Struct*

In-memory OTLP state for testing.

Records every operation as an [`OtlpEvent`]. No network is performed.

**Fields:**
- `events: std::sync::Mutex<Vec<OtlpEvent>>`

**Methods:**

- `fn new() -> Self` - Create a new stub.
- `fn events(self: &Self) -> Vec<OtlpEvent>` - All recorded events, in order.
- `fn record(self: &Self, event: OtlpEvent)`

**Trait Implementations:**

- **Default**
  - `fn default() -> StubOtlpState`
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



