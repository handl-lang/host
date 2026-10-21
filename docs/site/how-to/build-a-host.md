# How to build a host

This guide shows you how to create an executable host that runs a
capability — using only the public `handl-platform` crate.

## Goal

A Rust binary that:

1. Creates a `DefaultHost`.
2. Registers a plugin that provides a capability.
3. Loads the host.
4. Executes the capability.

## Steps

### 1. Add dependencies

```toml
[dependencies]
handl-platform = { git = "https://github.com/handl-lang/host" }
serde_json = "1"
```

### 2. Write a plugin and run it

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

### 3. Compose with kits and config

For real compositions, a `HostApplier` (from `handl-plugin-applier`)
reads `.handl.toml`, CLI flags, and env into a `HostConfig`:

```toml
[[kit]]
include = "my-kit"
exclude = ["my-experimental-plugin"]

[[plugin]]
manifest = "path/to/plugin.json"

[options]
"comment.style" = "above"
```

- `[[kit]]` — kits to load via the `kit/load` capability.
- `[[plugin]]` — external plugins to load via the `load` capability, by
  `PluginManifest` path.
- `[options]` — option overrides keyed by dotted option id.

## If something goes wrong

- **Unknown capability** — the plugin that provides it was not loaded,
  or its `capabilities()` slice does not include the id.
- **Missing dependency** — a plugin declared `requires` a capability no
  loaded plugin provides. Add the required plugin.
- **Option validation fails** — a user override is outside
  `OptionSchema::possible_values`. Fix the value or relax the schema.

## Next steps

- Read [The composable host](../explanation/composable-host) for the
  full design.
