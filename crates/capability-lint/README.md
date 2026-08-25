# handl-capability-lint

Built-in `lint` capability for the Handl platform.

This plugin provides a deterministic, testable `lint` command that reports
success without invoking an external linter.

## Metadata

```json
{
  "command": "lint",
  "description": "Lint a Handl module"
}
```

## Run

Given a `CapabilityRequest` with `capability == "lint"`, the plugin returns:

```json
{ "linted": true }
```
