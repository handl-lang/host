# handl-ext-extension

Built-in `extension` management capability for the Handl platform.

This plugin manages the `ExtensionRegistry` through `add`, `remove`, and `info`
actions.

## Metadata

```json
{
  "command": "extension",
  "description": "Manage extensions"
}
```

## Actions

- `add` — registers a simple `Extension` from the payload `id` and `metadata`.
- `remove` — removes an extension by id.
- `info` — returns a list of registered extension ids.
