**handl_lib_otlp > state**

# Module: state

## Contents

**Structs**

- [`MetricOptions`](#metricoptions) - Options passed to [`OtlpState::record_metric`].
- [`OtlpConfig`](#otlpconfig) - Configuration for an OTLP exporter.
- [`StartSpanOptions`](#startspanoptions) - Options passed to [`OtlpState::start_span`].

**Enums**

- [`MetricKind`](#metrickind) - Kind of metric recorded by [`OtlpState::record_metric`].

**Traits**

- [`OtlpState`](#otlpstate) - Abstract OpenTelemetry state that supports spans, logs and metrics.

---

## handl_lib_otlp::state::MetricKind

*Enum*

Kind of metric recorded by [`OtlpState::record_metric`].

**Variants:**
- `Counter` - Monotonically increasing counter.
- `Gauge` - Gauge value.

**Methods:**

- `fn parse(s: &str) -> Option<Self>` - Parse a metric kind from a string.

**Traits:** Eq, Copy

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &MetricKind) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> MetricKind`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_lib_otlp::state::MetricOptions

*Struct*

Options passed to [`OtlpState::record_metric`].

**Fields:**
- `kind: MetricKind` - Counter or gauge.
- `attributes: std::collections::HashMap<String, serde_json::Value>` - Metric attributes.

**Methods:**

- `fn counter(attributes: HashMap<String, Value>) -> Self` - Counter with the given attributes.
- `fn gauge(attributes: HashMap<String, Value>) -> Self` - Gauge with the given attributes.

**Trait Implementations:**

- **Default**
  - `fn default() -> Self`
- **Clone**
  - `fn clone(self: &Self) -> MetricOptions`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_lib_otlp::state::OtlpConfig

*Struct*

Configuration for an OTLP exporter.

**Fields:**
- `endpoint: String` - Base OTLP endpoint, e.g. `http://localhost:4318`.
- `service_name: String` - Service name added as a `service.name` resource attribute.
- `resource_attributes: std::collections::HashMap<String, serde_json::Value>` - Additional resource attributes.
- `headers: std::collections::HashMap<String, String>` - Headers added to every OTLP/HTTP request.

**Methods:**

- `fn new<impl Into<String>, impl Into<String>>(endpoint: impl Trait, service_name: impl Trait) -> Self` - Create a minimal config from an endpoint and a service name.

**Trait Implementations:**

- **Default**
  - `fn default() -> OtlpConfig`
- **Clone**
  - `fn clone(self: &Self) -> OtlpConfig`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_lib_otlp::state::OtlpState

*Trait*

Abstract OpenTelemetry state that supports spans, logs and metrics.

**Methods:**

- `configure`: Configure the exporter.
- `start_span`: Start a new span and return its span id.
- `end_span`: End the span with the given id.
- `set_attribute`: Set an attribute on an existing span.
- `add_event`: Add a named event to an existing span.
- `record_log`: Record a log entry.
- `record_metric`: Record a metric data point.
- `flush`: Flush all buffered telemetry to the configured exporter.
- `shutdown`: Flush and release the exporter.



## handl_lib_otlp::state::StartSpanOptions

*Struct*

Options passed to [`OtlpState::start_span`].

**Fields:**
- `parent: Option<String>` - Parent span id, if any.
- `attributes: std::collections::HashMap<String, serde_json::Value>` - Initial span attributes.

**Trait Implementations:**

- **Default**
  - `fn default() -> StartSpanOptions`
- **Clone**
  - `fn clone(self: &Self) -> StartSpanOptions`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



