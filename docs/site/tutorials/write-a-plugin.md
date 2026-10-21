# Write a plugin

In this tutorial you will extend the host from [Build a
host](./build-a-host) with a plugin that **contributes** something at
load time — the mechanism every real plugin uses to add rules, options,
and more plugins to a composition.

## What you will learn

- How `Plugin::contribute` works.
- How to contribute an `OptionSchema` to the `OptionsPlugin`
  `RuleAcceptor`.
- Why `contribute::<P>` is checked at compile time.

## Prerequisites

- The `my-host` crate from [Build a host](./build-a-host).

## Step 1: Register the `OptionsPlugin`

`handl-platform` ships `OptionsPlugin` — the `RuleAcceptor` that
collects `OptionSchema` contributions and resolves them into
`ResolvedOptions`. Register it alongside your plugin in `main`:

```rust
use handl_platform::OptionsPlugin;

let mut host = DefaultHost::new();
host.register_plugin(OptionsPlugin::new());
host.register_plugin(GreetPlugin::new());
host.load()?;
```

## Step 2: Contribute an option schema

Give `GreetPlugin` a `contribute` method that declares a
`greeting.punctuation` option:

```rust
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    ContributionContext, OptionSchema, OptionScope, OptionsPlugin,
    PlatformError, Plugin, PluginContext,
};
use serde_json::json;

impl Plugin for GreetPlugin {
    fn name(&self) -> &str { "greet" }
    fn capabilities(&self) -> &[CapabilityId] { &self.capabilities }
    fn metadata(&self, _cap: &CapabilityId) -> Option<CapabilityMetadata> { None }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        ctx.contribute::<OptionsPlugin>(OptionSchema {
            id: "greeting.punctuation".to_string(),
            scope: OptionScope::Global,
            applies_to: Some("greet".to_string()),
            default: json!("!"),
            possible_values: vec![json!("!"), json!("."), json!("…")],
            description: "How to end the greeting".to_string(),
        })?;
        Ok(())
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        // ... same as before
    }
}
```

`contribute::<OptionsPlugin>` is checked at compile time: the rule must
be an `OptionSchema`, because that is `OptionsPlugin`'s `Rule`
associated type. A `Format` rule cannot be contributed here by accident.

## Step 3: Build and run

```sh
cargo run
```

The option schema is merged into the resolved options during
`host.load()`. Values outside `possible_values` are rejected, so an
invalid `.handl.toml` override fails early with a clear error.

## What happened

1. `OptionsPlugin` was registered as a `RuleAcceptor<Rule =
   OptionSchema>`.
2. During `load`, `GreetPlugin::contribute` ran and its
   `OptionSchema` was accepted by `OptionsPlugin` — deferred and applied
   automatically if the acceptor had not been registered yet.
3. `OptionsPlugin` merged the schema default with kit and user
   overrides into `ResolvedOptions`, which every operation request can
   read.

## Contributing other rule kinds

The same pattern covers every rule kind — swap the `RuleAcceptor` type
parameter:

- `contribute::<BasicFormatter>(Box::new(my_format_behavior))` for
  `Format` rules.
- `contribute::<BasicLinter>(Box::new(my_lint_behavior))` for `Lint`
  rules.
- `ctx.register_plugin(MyOtherPlugin)` for library plugins that load
  more plugins.

Each call is type-checked against the acceptor's `Rule` type, so the
composition stays coherent no matter how many plugins join in.

## Next steps

- See [How to write a plugin](../how-to/write-a-plugin) for the
  condensed recipe.
- Read [Plugins, rules, and
  operations](../explanation/plugins-rules-operations) to understand the
  model in depth.
