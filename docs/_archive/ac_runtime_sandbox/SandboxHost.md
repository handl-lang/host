**ac_runtime_sandbox > SandboxHost**

# Module: SandboxHost

## Contents

**Functions**

- [`new_context`](#new_context) - Creates a [`Context`] with the sandbox limits applied.

---

## ac_runtime_sandbox::SandboxHost::new_context

*Function*

Creates a [`Context`] with the sandbox limits applied.

The VM will enforce `max_depth` and `max_calls` during evaluation,
returning [`Error::ResourceLimit`](ac_runtime::Error::ResourceLimit) if
either limit is exceeded.

```rust
fn new_context(self: &Self) -> CanonicalContext
```



