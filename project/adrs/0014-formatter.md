# handl Formatter

## Context

handl documents can be written in multiple frontends (YAML, JSON, TOML,
`.handl`). Within each frontend, the same logical document can be formatted in
many different styles: extra blank lines, mixed indentation, keys in
arbitrary order, trailing whitespace. This creates noisy diffs, harder
reviews, and formatting arguments in pull requests.

A language-aware formatter — like `gofmt`, `rustfmt`, or `prettier` —
enforces a canonical style. Unlike generic formatters, an handl formatter
understands the document model: field declarations, schemas, expressions,
imports, annotations. It can enforce canonical key order that matches the
language semantics, not just alphabetical or insertion order.

## Decision

handl provides an `handl fmt` command that formats documents in the
canonical style.

### Command surface

```bash
handl fmt document.yaml              # print formatted output to stdout
handl fmt document.yaml --write      # overwrite the file in place
handl fmt document.yaml --check      # exit non-zero if not already formatted
handl fmt libraries/**/*.yaml --write  # format many files
```

### Canonical style

1. **Two-space indentation** (YAML, `.handl`). JSON and TOML follow their
   native conventions.
2. **A single trailing newline** at the end of each file.
3. **No trailing whitespace** on any line.
4. **Canonical key order for mappings.** Unknown keys are kept after known
   keys in their original relative order.
   - Document top-level: `annotations`, `version`, `import`, `fields`,
     `functions`, `data`.
   - Field mapping: `type`, `expr`, `arg`, `schema`, `inheritable`,
     `mutable`, `merge`.
5. **Array order is preserved.** Authors may group items intentionally.
6. **Comments are preserved** where the underlying document model supports
   it.

### Frontend-specific behavior

The formatter dispatches based on file extension:

| Extension | Formatter |
|---|---|
| `.yaml` / `.yml` | YAML formatter (parse, reorder keys, stringify) |
| `.json` | JSON formatter (parse, reorder keys, pretty-print) |
| `.toml` | TOML formatter (parse, reorder keys, stringify) |
| `.handl` | `.handl` formatter (parse AST, emit canonical syntax) |

Each frontend's formatter shares the same canonical key order and style
rules, but implements them through its own parse/stringify pipeline.

### Implementation

- The formatter operates on the parsed document, re-orders mapping keys with
  a schema-aware sorter, then stringifies with the canonical rules.
- `--check` compares the emitted text to the original and exits `1` when
  they differ, but does not write anything.
- `--write` overwrites the source file atomically.
- The formatter does not resolve imports or merge included files — it
  formats only the file being edited.

### Plugin architecture

The formatter is part of the handl toolchain, not a host-specific feature.
Hosts and applications (e.g. Peregrin) can extend the canonical key order
with host-specific keys via a plugin configuration, but the core formatter
is language-level.

## Consequences

- All handl documents can be formatted the same way, reducing style noise
  in diffs.
- `handl fmt --check` can run in CI to enforce formatting.
- The formatter stays in sync with the language model, so language changes
  can update the canonical style in one place.
- Existing documents need a one-time pass with `handl fmt --write`.
- Each frontend needs its own parse/stringify implementation, but the key
  ordering logic is shared.

## Status

Proposed
