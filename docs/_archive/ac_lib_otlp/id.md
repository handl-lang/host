**handl_lib_otlp > id**

# Module: id

## Contents

**Functions**

- [`gen_span_id`](#gen_span_id) - Generates an 8-byte (16 hex chars) OTLP span id.
- [`gen_trace_id`](#gen_trace_id) - Generates a 16-byte (32 hex chars) OTLP trace id.
- [`now_ns`](#now_ns) - Current time as Unix nanoseconds.

---

## handl_lib_otlp::id::gen_span_id

*Function*

Generates an 8-byte (16 hex chars) OTLP span id.

```rust
fn gen_span_id() -> String
```



## handl_lib_otlp::id::gen_trace_id

*Function*

Generates a 16-byte (32 hex chars) OTLP trace id.

```rust
fn gen_trace_id() -> String
```



## handl_lib_otlp::id::now_ns

*Function*

Current time as Unix nanoseconds.

```rust
fn now_ns() -> u128
```



