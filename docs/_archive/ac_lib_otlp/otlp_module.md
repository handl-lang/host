**handl_lib_otlp > otlp_module**

# Module: otlp_module

## Contents

**Structs**

- [`AddEventFn`](#addeventfn)
- [`ConfigureFn`](#configurefn)
- [`EndSpanFn`](#endspanfn)
- [`FlushFn`](#flushfn)
- [`OtlpModule`](#otlpmodule) - Module adapter for [`OtlpState`] — exposes `otlp.*` functions.
- [`RecordLogFn`](#recordlogfn)
- [`RecordMetricFn`](#recordmetricfn)
- [`SetAttributeFn`](#setattributefn)
- [`ShutdownFn`](#shutdownfn)
- [`StartSpanFn`](#startspanfn)

**Functions**

- [`get_string`](#get_string)
- [`optional_object_at`](#optional_object_at)
- [`parse_attributes`](#parse_attributes)
- [`parse_config`](#parse_config)
- [`parse_headers`](#parse_headers)
- [`require_number_at`](#require_number_at)
- [`require_object_at`](#require_object_at)
- [`require_string_at`](#require_string_at)
- [`value_map_to_hashmap`](#value_map_to_hashmap)

---

## handl_lib_otlp::otlp_module::AddEventFn

*Struct*

**Fields:**
- `state: std::sync::Arc<dyn OtlpState>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, VmError>`



## handl_lib_otlp::otlp_module::ConfigureFn

*Struct*

**Fields:**
- `state: std::sync::Arc<dyn OtlpState>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, VmError>`



## handl_lib_otlp::otlp_module::EndSpanFn

*Struct*

**Fields:**
- `state: std::sync::Arc<dyn OtlpState>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, VmError>`



## handl_lib_otlp::otlp_module::FlushFn

*Struct*

**Fields:**
- `state: std::sync::Arc<dyn OtlpState>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, _args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, VmError>`



## handl_lib_otlp::otlp_module::OtlpModule

*Struct*

Module adapter for [`OtlpState`] — exposes `otlp.*` functions.

| Function | Args | Description |
|----------|------|-------------|
| `otlp.configure(config)` | `(config)` | Set endpoint, service name, headers and resource attributes. |
| `otlp.start_span(name, options?)` | `(name, options?)` | Start a span and return its id. |
| `otlp.end_span(id)` | `(id)` | End a span. |
| `otlp.set_attribute(id, key, value)` | `(id, key, value)` | Set a span attribute. |
| `otlp.add_event(id, name, attrs?)` | `(id, name, attrs?)` | Add a named event to a span. |
| `otlp.record_log(level, message, attrs?)` | `(level, message, attrs?)` | Record a log entry. |
| `otlp.record_metric(name, value, options?)` | `(name, value, options?)` | Record a numeric metric. |
| `otlp.flush()` | `()` | Export all buffered telemetry. |
| `otlp.shutdown()` | `()` | End all spans, flush and release the exporter. |

**Fields:**
- `configure_fn: ConfigureFn`
- `start_span_fn: StartSpanFn`
- `end_span_fn: EndSpanFn`
- `set_attr_fn: SetAttributeFn`
- `add_event_fn: AddEventFn`
- `record_log_fn: RecordLogFn`
- `record_metric_fn: RecordMetricFn`
- `flush_fn: FlushFn`
- `shutdown_fn: ShutdownFn`

**Methods:**

- `fn new(state: Arc<dyn OtlpState>) -> Self` - Create a new module backed by the given state.

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`



## handl_lib_otlp::otlp_module::RecordLogFn

*Struct*

**Fields:**
- `state: std::sync::Arc<dyn OtlpState>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, VmError>`



## handl_lib_otlp::otlp_module::RecordMetricFn

*Struct*

**Fields:**
- `state: std::sync::Arc<dyn OtlpState>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, VmError>`



## handl_lib_otlp::otlp_module::SetAttributeFn

*Struct*

**Fields:**
- `state: std::sync::Arc<dyn OtlpState>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, VmError>`



## handl_lib_otlp::otlp_module::ShutdownFn

*Struct*

**Fields:**
- `state: std::sync::Arc<dyn OtlpState>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, _args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, VmError>`



## handl_lib_otlp::otlp_module::StartSpanFn

*Struct*

**Fields:**
- `state: std::sync::Arc<dyn OtlpState>`

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, VmError>`



## handl_lib_otlp::otlp_module::get_string

*Function*

```rust
fn get_string<'a>(obj: &'a serde_json::Map<String, serde_json::Value>, key: &str) -> Option<&'a str>
```



## handl_lib_otlp::otlp_module::optional_object_at

*Function*

```rust
fn optional_object_at(args: &[ac_runtime_values::NonFunctionValue], idx: usize) -> Option<&serde_json::Map<String, serde_json::Value>>
```



## handl_lib_otlp::otlp_module::parse_attributes

*Function*

```rust
fn parse_attributes(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> std::collections::HashMap<String, serde_json::Value>
```



## handl_lib_otlp::otlp_module::parse_config

*Function*

```rust
fn parse_config(args: &[ac_runtime_values::NonFunctionValue]) -> Result<crate::state::OtlpConfig, ac_runtime_values::Error>
```



## handl_lib_otlp::otlp_module::parse_headers

*Function*

```rust
fn parse_headers(map: &serde_json::Map<String, serde_json::Value>) -> Result<std::collections::HashMap<String, String>, ac_runtime_values::Error>
```



## handl_lib_otlp::otlp_module::require_number_at

*Function*

```rust
fn require_number_at(args: &[ac_runtime_values::NonFunctionValue], idx: usize) -> Result<f64, ac_runtime_values::Error>
```



## handl_lib_otlp::otlp_module::require_object_at

*Function*

```rust
fn require_object_at(args: &[ac_runtime_values::NonFunctionValue], idx: usize) -> Result<&serde_json::Map<String, serde_json::Value>, ac_runtime_values::Error>
```



## handl_lib_otlp::otlp_module::require_string_at

*Function*

```rust
fn require_string_at(args: &[ac_runtime_values::NonFunctionValue], idx: usize) -> Result<&str, ac_runtime_values::Error>
```



## handl_lib_otlp::otlp_module::value_map_to_hashmap

*Function*

```rust
fn value_map_to_hashmap(map: &serde_json::Map<String, serde_json::Value>) -> std::collections::HashMap<String, serde_json::Value>
```



