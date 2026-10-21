# How to write a plugin

This guide shows you how to create a plugin that contributes a rule to a
`RuleAcceptor` — the type-safe extension point every operation uses.

## Goal

A plugin that declares a `greeting.punctuation` option via the
`OptionsPlugin` acceptor.

## Steps

### 1. Depend on the platform

```toml
[dependencies]
handl-platform = { git = "https://github.com/handl-lang/host" }
serde_json = "1"
```

### 2. Implement `Plugin`

```rust
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    ContributionContext, OptionSchema, OptionScope, OptionsPlugin,
    PlatformError, Plugin, PluginContext,
};
use serde_json::json;

pub struct GreetPlugin {
    capabilities: Vec<CapabilityId>,
}

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
            possible_values: vec![json!("!"), json!(".")],
            description: "How to end the greeting".to_string(),
        })?;
        Ok(())
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Ok(CapabilityResponse(json!("Hello!")))
    }
}
```

`contribute::<OptionsPlugin>` is checked at compile time — the rule must
match the acceptor's `Rule` associated type (`OptionSchema` here).

### 3. Register the plugin and the acceptor

```rust
host.register_plugin(OptionsPlugin::new());
host.register_plugin(GreetPlugin::new());
host.load()?;
```

## Common cases

Swap the `RuleAcceptor` type parameter for other rule kinds:

- `contribute::<BasicFormatter>(Box::new(rule))` for `Format` rules.
- `contribute::<BasicLinter>(Box::new(rule))` for `Lint` rules.
- `contribute::<BasicEvaluator>(rule)` for `Eval` rules.
- `ctx.register_plugin(plugin)` for library plugins that register more
  plugins.

If the acceptor is not registered yet, the rule is deferred and applied
as soon as it appears — contribution order does not matter.
