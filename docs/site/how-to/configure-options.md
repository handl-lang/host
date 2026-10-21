# How to configure options

This guide shows you how to declare an option from a plugin and override it from `.handl.toml`.

## Goal

A plugin that declares an option and a rule that reads it.

## Declare an option schema

Contribute an `OptionSchema` to the `Options` `RuleAcceptor`:

```rust
use handl_platform::{ContributionContext, OptionSchema, OptionsPlugin, PlatformError, Plugin};

pub struct CommentStylePlugin;

impl Plugin for CommentStylePlugin {
    fn id(&self) -> &str { "comment-style" }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        ctx.contribute::<OptionsPlugin>(OptionSchema {
            id: "comment.style".to_string(),
            scope: handl_platform::OptionScope::Global,
            applies_to: Some("comment".to_string()),
            default: serde_json::Value::String("above".to_string()),
            possible_values: vec![
                serde_json::Value::String("above".to_string()),
                serde_json::Value::String("inline".to_string()),
            ],
            description: "Where to place comments".to_string(),
        })?;

        Ok(())
    }
}
```

## Override from `.handl.toml`

```toml
[[kit]]
include = "my-kit"

[options]
"comment.style" = "inline"
```

## Read the resolved value in a rule

```rust
use handl_platform::{Behavior, Format, FormatRequest, PlatformError};

pub struct CommentFormatter;

impl Behavior<Format> for CommentFormatter {
    fn kind(&self) -> &str { "comment" }

    fn apply(&self, request: &mut FormatRequest) -> Result<(), PlatformError> {
        let style = request
            .options
            .choice("comment.style")
            .unwrap_or("above");

        match style {
            "above" => { /* ... */ }
            "inline" => { /* ... */ }
            _ => unreachable!(),
        }

        Ok(())
    }
}
```

## Resolution order

Resolved option values are produced by merging, from lowest to highest priority:

1. The plugin's declared default.
2. The kit's default value, if any.
3. The `.handl.toml` or `HostConfig` override.
4. Command-line flags, when supported by the `HostApplier`.

Higher-priority sources win. Values outside `possible_values` are rejected.
