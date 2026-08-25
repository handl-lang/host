# handl-capability-fmt

Built-in `fmt` capability for the Handl platform.

This plugin provides a deterministic, testable `fmt` command that reports
success without invoking an external formatter.

## Metadata

```json
{
  "command": "fmt",
  "description": "Format a Handl module"
}
```

## Run

Given a `CapabilityRequest` with `capability == "fmt"`, the plugin returns:

```json
{ "fmtd": true }
```
