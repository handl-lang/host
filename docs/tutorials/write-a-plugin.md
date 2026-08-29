# Write a plugin

In this tutorial you will extend the host from [Build a host](./build-a-host) with a custom plugin. The plugin will contribute a `CliCommand` that greets the user.

## What you will learn

- How to implement the `Plugin` trait.
- How to contribute a `CliCommand` to the `Cli` `RuleAcceptor`.
- How to register your plugin in a host.

## Prerequisites

- The `my-host` crate from [Build a host](./build-a-host).
- `handl-ext-cli` available locally.

## Step 1: Add a new crate for the plugin

Plugins usually live in their own crates so they can be reused across hosts. Create one:

```sh
cargo new --lib my-greet-plugin
cd my-greet-plugin
```

Add the dependencies in `Cargo.toml`:

```toml
[dependencies]
handl-platform = { path = "../crates/platform" }
handl-ext-cli = { path = "../crates/ext-cli" }
serde_json = "1"
```

## Step 2: Implement the `Plugin` trait

In `src/lib.rs`, write a plugin that contributes a `CliCommand`:

```rust
use handl_ext_cli::{Cli, CliCommand};
use handl_platform::{ContributionContext, PlatformError, Plugin};
use serde_json::json;

pub struct GreetPlugin;

impl Plugin for GreetPlugin {
    fn id(&self) -> &str {
        "my-greet"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        ctx.contribute::<Cli>(CliCommand {
            name: "greet".to_string(),
            about: "Greet someone".to_string(),
            run: Box::new(|host, matches| {
                let name = matches
                    .get_one::<String>("name")
                    .cloned()
                    .unwrap_or_else(|| "world".to_string());

                host.execute(
                    &"eval".into(),
                    json!({"expr": format!("\"Hello, {}!\"", name)}),
                )
            }),
        })?;

        Ok(())
    }
}
```

`contribute::<Cli>` is type-checked at compile time. The rule must match the `Rule` associated type of the `Cli` `RuleAcceptor`.

## Step 3: Register the plugin in the host

In `my-host/src/main.rs`, add the plugin to the host:

```rust
use my_greet_plugin::GreetPlugin;

// After applying the config and before loading:
host.register_plugin(GreetPlugin);
```

Your `main.rs` should look like this:

```rust
use handl_ext_cli::BasicCli;
use handl_platform::{ConfigHostApplier, Host, PlatformError};
use my_greet_plugin::GreetPlugin;
use serde_json::json;

fn main() -> Result<(), PlatformError> {
    let mut host = Host::new();

    let applier = ConfigHostApplier::new(std::env::args())?;
    applier.apply(&mut host)?;

    host.register_plugin(BasicCli::new());
    host.register_plugin(GreetPlugin);

    host.load()?;

    host.execute(
        &"cli".into(),
        json!({"args": std::env::args().collect::<Vec<_>>()}),
    )?;

    Ok(())
}
```

## Step 4: Build and run

Build the host:

```sh
cd ../my-host
cargo build
```

You should see the new command in the help:

```sh
./target/debug/my-host --help
```

Run it:

```sh
./target/debug/my-host greet Alice
```

Output:

```
"Hello, Alice!"
```

## What happened

1. `GreetPlugin` implemented `Plugin` and `contribute`.
2. `contribute::<Cli>` gave a `CliCommand` rule to `BasicCli`.
3. `BasicCli` used all `CliCommand` rules to build the `clap` command tree.
4. When you ran `greet Alice`, the CLI matched the command and its `run` closure called the `eval` capability.

## Next steps

- See [How to write a plugin](../how-to/write-a-plugin) for a shorter, copy-paste recipe.
- Read [Plugins, rules, and operations](../explanation/plugins-rules-operations) to understand the model in depth.
