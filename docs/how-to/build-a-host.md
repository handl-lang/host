# How to build a host

This guide shows you how to create an executable host that loads a kit and runs a capability.

## Goal

A Rust binary that:

1. Creates a `Host`.
2. Applies a `HostConfig` from configuration.
3. Registers an operation plugin.
4. Loads the host.
5. Executes a capability.

## Steps

### 1. Add dependencies

In your host crate's `Cargo.toml`:

```toml
[dependencies]
handl-platform = { path = "../crates/platform" }
handl-kit-canonical = { path = "../crates/kit-canonical" }
handl-ext-cli = { path = "../crates/ext-cli" }
serde_json = "1"
```

### 2. Create and configure the host

```rust
use handl_ext_cli::BasicCli;
use handl_platform::{ConfigHostApplier, Host, PlatformError};

let mut host = Host::new();

let applier = ConfigHostApplier::new(std::env::args())?;
applier.apply(&mut host)?;

host.register_plugin(BasicCli::new());
```

### 3. Load and execute

```rust
use serde_json::json;

host.load()?;

host.execute(
    &"cli".into(),
    json!({"args": std::env::args().collect::<Vec<_>>()}),
)?;
```

## Full example

```rust
use handl_ext_cli::BasicCli;
use handl_platform::{ConfigHostApplier, Host, PlatformError};
use serde_json::json;

fn main() -> Result<(), PlatformError> {
    let mut host = Host::new();

    let applier = ConfigHostApplier::new(std::env::args())?;
    applier.apply(&mut host)?;

    host.register_plugin(BasicCli::new());
    host.load()?;

    host.execute(
        &"cli".into(),
        json!({"args": std::env::args().collect::<Vec<_>>()}),
    )?;

    Ok(())
}
```

## If something goes wrong

- **Unknown capability** — the plugin that provides the capability was not loaded. Check the kit manifest and `HostConfig` excludes.
- **Missing dependency** — a plugin declared `requires` a capability that no loaded plugin provides. Add the required plugin.
- **Fixed point does not converge** — a `Library` or `Target` plugin is registering new plugins on every pass. `contribute` must be idempotent.
- **Option validation fails** — a user override is not in `OptionSchema::possible_values`. Correct the value or relax the schema.

## Next steps

- Read [The composable host](../explanation/composable-host) for the full design.
- See [How to write a plugin](./write-a-plugin) for adding custom behaviour.
