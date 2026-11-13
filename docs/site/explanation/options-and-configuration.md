# Options and configuration

Options in handl are not declared by the host or by the formatter. They are contributed by the plugins that own them. The `OptionsPlugin` collects `OptionSchema` contributions and resolves them into `ResolvedOptions` values that operations can query.

## Option schemas

A plugin declares an option by contributing an `OptionSchema` to the `OptionsPlugin` acceptor:

```rust
pub struct OptionSchema {
    pub id: String,
    pub scope: OptionScope,          // Global | Kit | Node
    pub applies_to: Option<String>,
    pub default: serde_json::Value,
    pub possible_values: Vec<serde_json::Value>,
    pub description: String,
}
```

For example, a plugin might declare:

```rust
OptionSchema {
    id: "comment.style".to_string(),
    scope: OptionScope::Global,
    applies_to: Some("comment".to_string()),
    default: serde_json::Value::String("above".to_string()),
    possible_values: vec![
        serde_json::Value::String("above".to_string()),
        serde_json::Value::String("inline".to_string()),
    ],
    description: "Where to place comments".to_string(),
}
```

## Resolution

`OptionsPlugin` accepts `OptionSchema` rules during `contribute` (deferred
and retried like any other rule contribution) and exposes the `option/set`
capability for overrides. `OptionsPlugin::resolve` builds a
`ResolvedOptions` map by starting from each schema's `default` and
applying every override set through `option/set`:

```rust
pub struct ResolvedOptions { /* id → serde_json::Value */ }

impl ResolvedOptions {
    pub fn get(&self, id: &str) -> Option<&Value>;
    pub fn choice(&self, id: &str) -> Option<&str>;
    pub fn is_enabled(&self, rule_id: &str) -> bool;
}
```

Operation request types that carry options (e.g. `FormatRequest.options`)
hold a `ResolvedOptions`, so a `Behavior` reads the values it needs
directly from the request.

## Configuration files

A `.handl.toml` is the main user-facing configuration file. It lists kits, direct plugins, exclusions, option overrides, and `[dependencies]`/`[patch]` package tables:

```toml
[[kit]]
include = "my-kit"

[[plugin]]
manifest = "path/to/plugin.json"

[options]
"comment.style" = "above"
"no-unused-imports.level" = "error"
"experimental.lifetime-check" = false
```

`HostConfig::from_toml` parses this into a `HostConfig` (`kits`,
`plugins`, `excludes`, `options`, `dependencies`, `patch`). The applier
plugins in `plugin-applier-*` read the file and apply it: kits are loaded
through the `kit/load` capability, plugin manifests through `load`,
`[options]` entries through `option/set`, and excludes through
`ContributionContext::exclude_plugin`. `DefaultHost` itself never touches
the filesystem.
