# ac_runtime_sandbox

# handl-runtime-sandbox

A [`Host`] for running untrusted handl expressions in a sandbox.

Provides only pure (side-effect-free) library modules plus the built-in
[`DefaultHost`] functions. No I/O, no side effects, no `import()`.
Resource limits (scope depth, call count) are enforced via the
[`Context`] returned by [`SandboxHost::new_context`].

A host that provides only pure functions is automatically sandboxed —
this is a policy decision, not a language feature. The VM enforces
the resource limits; the host configures them.

## What's included

- **Built-ins** (`add`, `mul`, `concat`, `len`, `var`, `resolve`, `apply`, `array`)
- **`math.*`**, **`str.*`**, **`array.*`**, **`set.*`**, **`bool.*`**,
  **`cmp.*`**, **`obj.*`**, **`fn.*`**, **`type.*`** — from [`handl-lib-utils`]
- **`crypto.*`** — SHA-256, MD5, Base64, hex — from [`handl-lib-crypto`]
- **`datetime.*`** — parsing, formatting, arithmetic — from [`handl-lib-datetime`]
- **`intl.*`** — locale-aware formatting (ICU4X) — from [`handl-lib-intl`]
- **`kv.*`** — in-memory key-value store — from [`handl-lib-kv`] + [`handl-lib-kv-hashmap`]

## What's excluded

- [`handl-lib-rand`] — non-deterministic RNG
- [`handl-lib-os`] / [`handl-lib-os-std`] — filesystem, env,
  process spawning, system clock
- [`handl-lib-http-client`] / [`handl-lib-http-client-std`] — HTTP client

## Usage

```ignore
use ac_runtime_sandbox::{SandboxHost, SandboxLimits};
use ac_runtime_canonical::Vm;

let host = SandboxHost::new(SandboxLimits::default());
let ctx = host.new_context();
let vm = Vm::new(host);
```

With custom limits:

```ignore
use ac_runtime_sandbox::{SandboxHost, SandboxLimits};

let limits = SandboxLimits {
    max_depth: Some(50),
    max_calls: Some(1_000),
};
let host = SandboxHost::new(limits);
let ctx = host.new_context();
```

## Modules

### [`ac_runtime_sandbox`](ac_runtime_sandbox.md)

*2 structs*

### [`SandboxHost`](SandboxHost.md)

*1 function*

