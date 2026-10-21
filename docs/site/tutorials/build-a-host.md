# Build a host

In this tutorial you will build an executable handl host from scratch —
a Rust binary that creates an empty `Host`, registers a plugin that
provides a capability, loads the composition, and runs it.

## What you will learn

- How a `Host` starts as an empty composer.
- How plugins provide capabilities.
- How `load` resolves the plugin graph.
- How to execute a capability.

## Prerequisites

- A Rust toolchain.
- Follow [Set up the platform](./setup) first, or just create a fresh
  crate — the only dependency you need is the public
  `handl-platform` crate from the `host` repo.

## Step 1: Create a new binary crate

```sh
cargo new --bin my-host
cd my-host
```

## Step 2: Depend on the platform

```toml
[dependencies]
handl-platform = { git = "https://github.com/handl-lang/host" }
serde_json = "1"
```

`handl-platform` provides `DefaultHost`, `Plugin`, `CapabilityRequest`,
and the composition machinery. Nothing else is required — the host
starts empty.

## Step 3: Write a capability plugin

Replace `src/main.rs` with a plugin that answers a `greet` capability:

```rust
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    DefaultHost, PlatformError, Plugin, PluginContext,
};
use serde_json::json;

struct GreetPlugin {
    capabilities: Vec<CapabilityId>,
}

impl GreetPlugin {
    fn new() -> Self {
        Self { capabilities: vec!["greet".into()] }
    }
}

impl Plugin for GreetPlugin {
    fn name(&self) -> &str { "greet" }
    fn capabilities(&self) -> &[CapabilityId] { &self.capabilities }
    fn metadata(&self, _cap: &CapabilityId) -> Option<CapabilityMetadata> { None }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        let name = request
            .payload
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("world");
        Ok(CapabilityResponse(json!(format!("Hello, {name}!"))))
    }
}

fn main() -> Result<(), PlatformError> {
    let mut host = DefaultHost::new();
    host.register_plugin(GreetPlugin::new());
    host.load()?;

    let resp = host.run(&CapabilityRequest {
        capability: "greet".into(),
        payload: json!({"name": "alice"}),
    })?;

    println!("{}", resp.0);
    Ok(())
}
```

Notice that the host starts empty. Everything it can do is added by
plugins — here, a single plugin provides the entire `greet` capability
in `run`.

## Step 4: Build and run

```sh
cargo run
```

You should see:

```text
"Hello, alice!"
```

## What happened

1. `DefaultHost::new()` created an empty composer.
2. `register_plugin` added `GreetPlugin` to the `PluginRegistry`,
   declaring the `greet` capability.
3. `host.load()` resolved plugin dependencies and ran every plugin's
   `contribute` method to a fixed point (this plugin contributes
   nothing).
4. `host.run` dispatched the `CapabilityRequest` to the plugin providing
   `greet` — `GreetPlugin` — which returned a `CapabilityResponse`.

The same shape scales to real tools: kits register many plugins, plugins
contribute rules to each other's `RuleAcceptor`s, and the host dispatches
capabilities without knowing what tool it has become.

## Next steps

- In [Write a plugin](./write-a-plugin), you will add a rule-contribution
  path with `contribute::<P>` — the type-safe way plugins extend
  operations.
- In [How to build a host](../how-to/build-a-host), you will see a
  condensed recipe with fewer explanations.
- Read [The composable host](../explanation/composable-host) to learn why
  the host is designed this way.
