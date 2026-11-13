# handl-plugin-ext-lint

The `lint` capability plugin for the Handl platform — a thin
capability-contract adapter over the canonical linter.

`LintPlugin` (in `src/plugin.rs`) implements `handl_platform::Plugin` for
the `lint` capability, holding an injected `LintContext`
(the host's `Language` plus its `.hnl`-rule resolver, ADR-0128 §"The 0.8
seam"). The free functions `lint_source_with` / `check_source_with` /
`fix_source` remain the implementation; rules are `LintRuleDef`s in
`lang.lints` executed by the `hnl-engine-lint` engine.

## Payload contract (capability `lint`)

```json
{ "source": "<text>", "fix": false, "prelude": ["str", "map"],
  "language": { …serialized Language… } }
```

- `source` (required, string) — the text to lint.
- `fix` (optional, bool, default `false`) — return the fixed source
  instead of the diagnostics.
- `prelude` (optional, array of strings) — canonical wire names the
  checker should treat as bound.
- `language` (optional, serialized `Language`, ADR-0124) — overrides the
  injected context's language for this request.

## Response

Discriminated by `status`, matching the `fmt` capability:

- `{ "status": "diagnostics", "diagnostics": [ <Diagnostic>, … ] }`
- `{ "status": "fixed", "output": "<fixed text>" }`
- `{ "status": "error", "message": "<why>" }`

Only a malformed *request* is a `PlatformError`; a parse/lint failure is a
successful response carrying an error status. See `src/plugin.rs` for the
full contract.
