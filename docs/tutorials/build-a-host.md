# Build a host

In this tutorial you will build an executable handl host from scratch. You will create a Rust binary, load a kit, register a CLI operation, and run a capability.

## What you will learn

- How a `Host` is an empty composer.
- How to apply a `HostConfig` from a `HostApplier`.
- How to register an operation plugin and load the host.
- How to execute a capability.

## Prerequisites

- A Rust toolchain.
- The handl workspace checked out locally.
- Basic familiarity with Rust traits and crates.

## Step 1: Create a new binary crate

From the workspace root, create a host crate:

```sh
cargo new --bin my-host
cd my-host
```

## Step 2: Link to the platform crates

Open `Cargo.toml` and add the dependencies you need:

```toml
[dependencies]
handl-platform = { path = "../crates/platform" }
handl-kit-canonical = { path = "../crates/kit-canonical" }
handl-ext-cli = { path = "../crates/ext-cli" }
serde_json = "1"
```

`handl-platform` provides `Host` and `Plugin`. `handl-kit-canonical` provides the kit manifest. `handl-ext-cli` provides the `Cli` operation plugin.

## Step 3: Create a `Host`

Replace `src/main.rs` with:

```rust
use handl_ext_cli::BasicCli;
use handl_platform::{ConfigHostApplier, Host, PlatformError};
use serde_json::json;

fn main() -> Result<(), PlatformError> {
    let mut host = Host::new();

    // Load configuration from .handl.toml and CLI args.
    let applier = ConfigHostApplier::new(std::env::args())?;
    applier.apply(&mut host)?;

    // Register the CLI operation plugin.
    host.register_plugin(BasicCli::new());

    // Resolve dependencies, run contributions, and finalise options.
    host.load()?;

    // Execute the 'cli' capability.
    host.execute(
        &"cli".into(),
        json!({"args": std::env::args().collect::<Vec<_>>()}),
    )?;

    Ok(())
}
```

Notice that the host starts empty. Everything it can do is added by plugins.

## Step 4: Add a `.handl.toml`

Create `.handl.toml` next to `Cargo.toml`:

```toml
[[kit]]
include = "handl-kit-canonical"
```

This tells the applier to load the canonical kit, which contains the standard library, parser, lexer, and evaluator plugins.

## Step 5: Build and run

Build the binary:

```sh
cargo build
```

Run it with a simple command:

```sh
./target/debug/my-host --help
```

You should see the help text for the CLI, generated from the `CliCommand` rules contributed by the loaded plugins. Run a specific command:

```sh
./target/debug/my-host eval 42
```

You should see the evaluated result printed to `stdout`.

## What happened

1. `Host::new()` created an empty composer.
2. `ConfigHostApplier` read `.handl.toml` and applied a `HostConfig`.
3. `BasicCli` was registered as the operation that accepts `CliCommand` rules.
4. `Host::load()` resolved dependencies and ran `contribute` to a fixed point.
5. `BasicCli` collected the `CliCommand` rules from the kit and built the command tree.
6. `host.execute` dispatched the `cli` capability to `BasicCli`, which ran the matched command.

## Next steps

- In [Write a plugin](./write-a-plugin), you will create a plugin that contributes a new command to the CLI.
- In [How to build a host](../how-to/build-a-host), you will see a condensed recipe with fewer explanations.
- Read [The composable host](../explanation/composable-host) to learn why the host is designed this way.
