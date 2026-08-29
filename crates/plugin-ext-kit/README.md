# handl-ext-kit

Built-in `kit` management capability for the Handl platform.

This plugin manages the `KitRegistry` through `add`, `remove`, and `info`
actions.

## Metadata

```json
{
  "command": "kit",
  "description": "Manage kits"
}
```

## Actions

- `add` — registers a `Kit` from the payload `name` and `plugins`.
- `remove` — removes a kit by name.
- `info` — returns a list of registered kit names.
