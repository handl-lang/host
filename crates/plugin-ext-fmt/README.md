# handl-plugin-ext-fmt

The `fmt` capability plugin for the Handl platform — a thin
capability-contract adapter over the canonical formatter.

`FmtPlugin` (in `src/plugin.rs`) implements `handl_platform::Plugin` for
the `fmt` capability. The `Language` it formats against is injected at
construction (`FmtPlugin::new(Arc<Language>)`, ADR-0128 §"The 0.8 seam");
the free functions `format_source_with_language` / `check_source_with_language`
in this crate remain the implementation.

## Payload contract (capability `fmt`)

```json
{ "source": "<text>", "check": false }
```

- `source` (required, string) — the text to format.
- `check` (optional, bool, default `false`) — report whether `source` is
  already formatted instead of returning the formatted text.

## Response

A single `status` discriminates the outcome:

- `{ "status": "canonical" }` — `check: true`, input already canonical.
- `{ "status": "not-canonical" }` — `check: true`, input differs.
- `{ "status": "formatted", "output": "<text>" }` — `check: false`.
- `{ "status": "error", "diagnostic": { … } }` — could not parse/format;
  the structured `plugin-ext-diag` diagnostic is returned rather than an
  error string.

Only a malformed *request* is a `PlatformError` (bad payload, wrong
capability); a formatting failure is a successful response carrying a
diagnostic. See `src/plugin.rs` for the full contract.
