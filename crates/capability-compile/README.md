# handl-capability-compile

Built-in `compile` capability for the Handl platform.

This plugin provides a deterministic, testable `compile` command that reports
success without invoking a compiler.

## Metadata

```json
{
  "command": "compile",
  "description": "Compile a Handl module"
}
```

## Run

Given a `CapabilityRequest` with `capability == "compile"`, the plugin returns:

```json
{ "compiled": true }
```
