# ADR-0121: WebIDL Cross-Spec `includes` — Supplementary Mixin Specs

## Status

Accepted.

## Context

WHATWG/W3C specs routinely apply `X includes Y` where `Y` is an
`interface mixin` declared in a _different_ spec. Concretely, `encoding.idl`
declares:

    TextDecoderStream includes GenericTransformStream;
    TextEncoderStream includes GenericTransformStream;

but `GenericTransformStream` lives in `streams.idl`. The WebIDL backend
([ADR-0113](./0113-handl-sdkgen-webidl-js.md)) resolved `includes` only
against mixins in the same file, so cross-spec references surfaced as
diagnostics (`includes: mixin GenericTransformStream not found`) and the
generated records silently lacked the grafted members.

Two designs were considered:

1. **Multi-spec generation** — parse N idl files into one `SdkgenModule`.
   Merges unrelated surfaces into a single package and breaks the
   `<spec-stem>` → package-name convention (which stem wins?).
2. **Supplementary mixin specs** — supplementary files contribute _only_
   mixin definitions to `includes` resolution; their interfaces, records,
   typedefs, and constants are not emitted.

## Decision

Supplementary mixin specs, declared in the spec's `.sdkgen.yaml`:

```yaml
# handl-packages/generated/encoding.sdkgen.yaml
includes:
  - streams.idl
```

- `WebIdlConfig.includes: Vec<String>` — paths relative to the config
  file's directory (same reproducibility convention as the C backend's
  `headers:` overrides).
- `WebIdlBackend::add_include_spec(path)` registers each path; during
  `parse_header` the file is parsed and its `AstDef::Mixin` defs are added
  to the graft map. Primary-spec mixins win on a name collision.
- Diagnostics are preserved: supplementary parse diagnostics are tagged
  with the file path, unreadable files and still-missing mixins report
  exactly as before.
- Emission granularity is unchanged: one spec → one package. Encoding
  gains `readable`/`writable` on `TextDecoderStream`/`TextEncoderStream`;
  `streams.idl` types (`ReadableStream`, `WritableStream`) continue to
  map to `Handle` — no foreign records are pulled in.

## Consequences

- `encoding.sdkgen.yaml` lists `streams.idl`; the generated
  `TextDecoderStream`/`TextEncoderStream` records are now complete.
- Adding a spec that references foreign mixins requires an
  `includes:` entry — explicit, grep-able, and reproducible rather than
  ambient multi-file merging.
- Because spec inputs (`*.idl`/`*.webidl`) are gitignored, the
  `includes:` paths are documentation of the required fetch set, not
  vendored references.

Refs: `plugin-sdkgen-backend-webidl` (`includes_resolve_across_specs`
test), `plugin-ext-sdkgen` CLI wiring.
