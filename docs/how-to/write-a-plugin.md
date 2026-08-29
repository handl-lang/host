# How to write a plugin

This guide shows you how to create a plugin that contributes a `CliCommand` to the `Cli` operation.

## Goal

A plugin that adds a `greet` command to the CLI.

## Steps

### 1. Create a crate

```sh
cargo new --lib my-greet-plugin
```

Add dependencies:

```toml
[dependencies]
handl-platform = { path = "../crates/platform" }
handl-ext-cli = { path = "../crates/ext-cli" }
serde_json = "1"
```

### 2. Implement `Plugin`

```rust
use handl_ext_cli::{Cli, CliCommand};
use handl_platform::{ContributionContext, PlatformError, Plugin};
use serde_json::json;

pub struct GreetPlugin;

impl Plugin for GreetPlugin {
    fn id(&self) -> &str { "my-greet" }
    fn version(&self) -> &str { "0.1.0" }
    fn metadata(&self) -> serde_json::Value { serde_json::Value::Null }

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

### 3. Register the plugin in a host

```rust
use my_greet_plugin::GreetPlugin;

host.register_plugin(GreetPlugin);
host.load()?;
```

### 4. Run the command

```sh
./target/debug/my-host greet Alice
```

## Common cases

If you are contributing to a different operation, change the `RuleAcceptor`:

- `contribute::<Linter>(...)` for lint rules.
- `contribute::<Formatter>(...)` for format rules.
- `contribute::<Options>(...)` for option schemas.
- `contribute::<DataTypes>(...)` for data types.

Each call is type-checked at compile time against the `RuleAcceptor::Rule` associated type.
