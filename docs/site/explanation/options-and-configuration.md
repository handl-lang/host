# Options and configuration

Options in handl are not declared by the host or by the formatter. They are contributed by the plugins that own them. The `Options` plugin resolves the final values from schemas, kit defaults, and user overrides.

## Option schemas

A plugin declares an option by contributing an `OptionSchema`:

```rust
pub struct OptionSchema {
    pub id: String,
    pub scope: OptionScope,
    pub applies_to: Option<String>,
    pub default: serde_json::Value,
    pub possible_values: Vec<serde_json::Value>,
    pub description: String,
}
```

For example, a formatter plugin might declare:

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

The formatter and linter never know `comment.style` exists. They read the resolved value from the `ResolvedOptions` attached to their `Request`.

## Resolution order

The `Options` plugin resolves values by merging, from lowest to highest priority:

1. The `default` from the `OptionSchema`.
2. Kit default values from `KitManifest`.
3. `HostConfig` overrides, typically from `.handl.toml`.
4. CLI flags or environment variables from the `HostApplier`, when supported.

Higher-priority sources override lower ones. Any value outside `possible_values` is rejected with an error, so invalid values fail early.

## ResolvedOptions

`ResolvedOptions` is a map from option id to value. It is built once during `Host::load` and shared for the lifetime of the operation:

```rust
pub struct ResolvedOptions {
    values: HashMap<String, serde_json::Value>,
}

impl ResolvedOptions {
    pub fn get(&self, id: &str) -> Option<&Value>;
    pub fn choice(&self, id: &str) -> Option<&str>;
    pub fn is_enabled(&self, rule_id: &str) -> bool;
}
```

Every operation `Request` carries a `&ResolvedOptions`. A `Behavior` reads the option values it needs directly from the request.

## Why options are plugin-defined

If options lived in `Host` or `KitManifest`, adding a new option would require editing the host. Plugin-defined options keep the host ignorant of specific tunables. A formatter plugin can add `comment.style`, a linter plugin can add `comment.max-length`, and the `Options` plugin resolves both without the formatter or linter knowing about each other.

## Configuration files

A `.handl.toml` is the main user-facing configuration file. It lists kits, direct plugins, exclusions, and option overrides:

```toml
[[kit]]
include = "my-kit"

[[plugin]]
register = "my-custom-linter"

[options]
"comment.style" = "above"
"no-unused-imports.level" = "error"
"experimental.lifetime-check" = false
```

The `HostApplier` reads this file and CLI flags, builds a `HostConfig`, and applies it to the `Host`. The `Host` itself never touches the filesystem.
