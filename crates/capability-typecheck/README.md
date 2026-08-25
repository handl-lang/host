# handl-capability-typecheck

Built-in `typecheck` capability for the Handl platform.

This plugin provides a deterministic, testable `typecheck` command that reports
success without invoking a type checker.

## Metadata

```json
{
  "command": "typecheck",
  "description": "Typecheck a Handl module"
}
```

## Run

Given a `CapabilityRequest` with `capability == "typecheck"`, the plugin returns:

```json
{ "typechecked": true }
```
