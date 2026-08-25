**ac_runtime_common > sandbox**

# Module: sandbox

## Contents

**Functions**

- [`with_limits`](#with_limits) - Sets both resource limits on a context.
- [`with_max_calls`](#with_max_calls) - Sets the maximum call count on a context.
- [`with_max_depth`](#with_max_depth) - Sets the maximum scope depth on a context.

---

## ac_runtime_common::sandbox::with_limits

*Function*

Sets both resource limits on a context.

```rust
fn with_limits<C>(ctx: & mut C, max_depth: Option<usize>, max_calls: Option<usize>) -> & mut C
```



## ac_runtime_common::sandbox::with_max_calls

*Function*

Sets the maximum call count on a context.

```rust
fn with_max_calls<C>(ctx: & mut C, max: usize) -> & mut C
```



## ac_runtime_common::sandbox::with_max_depth

*Function*

Sets the maximum scope depth on a context.

```rust
fn with_max_depth<C>(ctx: & mut C, max: usize) -> & mut C
```



