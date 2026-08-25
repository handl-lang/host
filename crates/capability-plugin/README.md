# handl-capability-plugin

Built-in `plugin` management capability for the Handl platform.

This plugin manages the `PluginRegistry` through `add`, `remove`, and `info`
actions.

## Metadata

```json
{
  "command": "plugin",
  "description": "Manage plugins"
}
```

## Actions

- `add` — registers a `ProcessPlugin` from the payload `name` and `capabilities`.
- `remove` — currently returns `PlatformError::NotImplemented`.
- `info` — returns a list of registered plugin names.
