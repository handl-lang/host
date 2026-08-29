# How to build a host

This guide shows you how to create an executable host that loads a kit and runs a capability.

## Goal

A Rust binary that:

1. Creates a `DefaultHost`.
2. Registers a `BootstrapPlugin` that reads `.handl.toml` and selects an applier.
3. Registers an operation plugin.
4. Loads the host.
5. Executes a capability.

## Steps

### 1. Add dependencies

In your host crate's `Cargo.toml`:

```toml
[dependencies]
handl-platform = { path = "../crates/platform" }
handl-plugin-applier = { path = "../crates/plugin-applier" }
handl-plugin-ext-cli = { path = "../crates/plugin-ext-cli" }
serde_json = "1"
```

### 2. Create and configure the host

```rust
use handl_plugin_applier::BootstrapPlugin;
use handl_plugin_ext_cli::BasicCli;
use handl_platform::{DefaultHost, PlatformError};

let mut host = DefaultHost::new();

host.register_plugin(BootstrapPlugin::new(".handl.toml", "hello", "applier.default"));
host.register_plugin(BasicCli::new());
```

`BootstrapPlugin` reads `.handl.toml` (if present), selects the applier by id, and registers it. The applier then loads kits via the `kit/load` capability and plugin manifests via the `load` capability during `host.load()`.

### 3. Load and execute

```rust
use handl_platform::CapabilityRequest;
use serde_json::json;

host.load()?;

let resp = host.run(&CapabilityRequest {
    capability: "cli".into(),
    payload: json!({"command": "eval", "args": ["hello.greet(\"world\")"]}),
})?;
```

## Full example

```rust
use handl_plugin_applier::BootstrapPlugin;
use handl_plugin_ext_cli::BasicCli;
use handl_platform::{CapabilityRequest, DefaultHost, PlatformError};
use serde_json::json;

fn main() -> Result<(), PlatformError> {
    let mut host = DefaultHost::new();

    host.register_plugin(BootstrapPlugin::new(".handl.toml", "hello", "applier.default"));
    host.register_plugin(BasicCli::new());
    host.load()?;

    let resp = host.run(&CapabilityRequest {
        capability: "cli".into(),
        payload: json!({"command": "eval", "args": ["hello.greet(\"world\")"]}),
    })?;

    if let Some(s) = resp.0.as_str() {
        println!("{s}");
    }
    Ok(())
}
```

## CLI flags

The reference `handl-host-cli` binary supports:

- `--applier <id>` — select the applier plugin (default `applier.default`).
- `--config <path>` — path to the host config file (default `.handl.toml`).
- `--kit <name>` — fallback kit when no `[[kit]]` entries are in the config (default from `HANDL_KIT` env var or `hello`).

## Configuration file

`.handl.toml` is a TOML file with three sections:

```toml
[[kit]]
include = "canonical"
exclude = ["handl-plugin-deprecated-grammar"]

[[plugin]]
manifest = "path/to/plugin.json"

[options]
"comment.style" = "above"
```

- `[[kit]]` — kits to load via the `kit/load` capability.
- `[[plugin]]` — external plugins to load via the `load` capability. Use `manifest` for a path to a `PluginManifest` JSON file.
- `[options]` — option overrides keyed by dotted option id.

## If something goes wrong

- **Unknown capability** — the plugin that provides the capability was not loaded. Check the kit manifest and `HostConfig` excludes.
- **Missing dependency** — a plugin declared `requires` a capability that no loaded plugin provides. Add the required plugin.
- **Unknown applier** — the `--applier` id does not match any registered applier. Use `applier.default` or register a custom one.
- **Option validation fails** — a user override is not in `OptionSchema::possible_values`. Correct the value or relax the schema.

## Next steps

- Read [The composable host](../explanation/composable-host) for the full design.
