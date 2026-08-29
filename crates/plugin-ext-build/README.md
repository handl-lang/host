# handl-ext-build

Built-in `build` capability for the Handl platform.

This plugin provides a deterministic, testable `build` command that reports
success without invoking an external build tool.

## Metadata

```json
{
  "command": "build",
  "description": "Build a Handl project"
}
```

## Run

Given a `CapabilityRequest` with `capability == "build"`, the plugin returns:

```json
{ "built": true }
```
