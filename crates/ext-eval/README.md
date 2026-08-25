# handl-ext-eval

Built-in `eval` capability for the Handl platform.

This plugin provides a deterministic, testable `eval` command that reports
success without invoking an evaluator.

## Metadata

```json
{
  "command": "eval",
  "description": "Evaluate a Handl expression"
}
```

## Run

Given a `CapabilityRequest` with `capability == "eval"`, the plugin returns:

```json
{ "evaled": true }
```
