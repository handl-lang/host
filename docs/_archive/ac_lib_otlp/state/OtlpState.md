**handl_lib_otlp > state > OtlpState**

# Module: state::OtlpState

## Contents

**Functions**

- [`record_metric`](#record_metric) - Record a metric data point.
- [`start_span`](#start_span) - Start a new span and return its span id.

---

## handl_lib_otlp::state::OtlpState::record_metric

*Function*

Record a metric data point.

```rust
fn record_metric(self: &Self, name: &str, value: f64, options: MetricOptions) -> OtlpResult<()>
```



## handl_lib_otlp::state::OtlpState::start_span

*Function*

Start a new span and return its span id.

```rust
fn start_span(self: &Self, name: &str, options: StartSpanOptions) -> OtlpResult<String>
```



