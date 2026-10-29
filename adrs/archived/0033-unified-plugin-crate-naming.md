---
title: Unified Plugin Crate Naming
type: adr
date: 2026-08-29
status: accepted
---

# ADR-0033: Unified Plugin Crate Naming

## Status

Accepted

## Context

The Handl platform has 79 `plugin-*` directories under `crates/`. Their
names have grown organically and are now inconsistent, which makes the
workspace hard to navigate, sort, and keep in `Cargo.toml`. Only
`plugin-comments` and `plugin-doc-comments` were listed in the
root `[workspace] members` at the time of writing; the rest were only in
`[workspace.dependencies]` or missing entirely.

Current naming patterns:

| pattern              | count |
| -------------------- | ----- |
| `plugin-ast-*`       | 13    |
| `plugin-grammar-*`   | 10    |
| `plugin-token-*`     | 10    |
| `plugin-primary-*`   | 9     |
| `plugin-infix-*`     | 5     |
| `plugin-stmt-*`      | 5     |
| `plugin-postfix-*`   | 4     |
| `plugin-fmt-*`       | 3     |
| `plugin-lsp-*`       | 3     |
| `plugin-diag-*`      | 1     |
| `plugin-typecheck-*` | 1     |
| `plugin-vm-*`        | 1     |
| bare `plugin-<name>` | 14    |

Notable inconsistencies:

- Token plugins are split between bare crates (`plugin-booleans`,
  `plugin-numbers`, `plugin-strings`, `plugin-null`, `plugin-bytes`,
  `plugin-identifiers`, `plugin-keywords`, `plugin-operators`,
  `plugin-delimiters`) and `plugin-token-*` crates
  (`plugin-token-bool`, `plugin-token-number`, `plugin-token-comment`,
  etc.).
- Near-duplicates exist, e.g. `plugin-comment-tokens` vs
  `plugin-token-comment`.
- Plural and singular forms are mixed, e.g. `plugin-primary-arrays`
  vs. `plugin-ast-array`.
- Parser-shape namespaces overlap: `plugin-primary-*`,
  `plugin-postfix-*`, `plugin-infix-*`, and `plugin-stmt-*` all live at
  the top level.
- Backend/target plugins use mixed conventions, e.g. `plugin-vm-rust`
  and `native-plugin-example` (whose package name is
  `native-plugin-example`).

A single naming convention is needed before the workspace manifests and
kit registries can be maintained automatically.

## Decision

Every plugin crate will use the directory name

```
plugin-<domain>-<construct>
```

and the Cargo package name

```
handl-plugin-<domain>-<construct>
```

Approved domains:

| domain      | meaning                          |
| ----------- | -------------------------------- |
| `token`     | lexer token kinds                |
| `grammar`   | grammar rules and parser shapes  |
| `ast`       | AST node shapes                  |
| `fmt`       | formatter behaviors              |
| `diag`      | diagnostics                      |
| `lsp`       | LSP features                     |
| `typecheck` | type checker rules and behaviors |
| `target`    | codegen / VM / backend plugins   |

Rules:

1. Construct names are **singular**:
   `bool`, `number`, `string`, `identifier`, `operator`, `delimiter`,
   `keyword`, `comment`, `call`, `array`, `object`, `stmt`, `let`,
   `const`, `import`, `export`, `module`, `index`, `field`, `pipe`,
   `assign`, etc.
2. Bare token/keyword/operator/identifier/boolean/number/string/comment/
   bytes/null crates collapse into `plugin-token-*`.
3. Parser-shape crates (`primary`, `postfix`, `infix`, `stmt`) move
   under `plugin-grammar-*` with the form
   `plugin-grammar-<shape>-<construct>`.
4. Plurals become singular, e.g. `plugin-primary-arrays` becomes
   `plugin-grammar-primary-array`.
5. Near-duplicates are merged or removed; the canonical token remains
   `plugin-token-comment`.
6. Backends use the `target` domain:
   - `plugin-vm-rust` -> `plugin-target-rust`
   - `plugin-native-example` -> `plugin-target-native-example`
7. Once a crate follows the naming scheme it is added to both the root
   `[workspace] members` and, when used by other crates,
   `[workspace.dependencies]`.

### Example mappings

| current directory            | proposed directory                  | current package                    | proposed package                          |
| ---------------------------- | ----------------------------------- | ---------------------------------- | ----------------------------------------- |
| `plugin-booleans`            | `plugin-token-bool`                 | `handl-plugin-booleans`            | `handl-plugin-token-bool`                 |
| `plugin-numbers`             | `plugin-token-number`               | `handl-plugin-numbers`             | `handl-plugin-token-number`               |
| `plugin-strings`             | `plugin-token-string`               | `handl-plugin-strings`             | `handl-plugin-token-string`               |
| `plugin-bytes`               | `plugin-token-bytes`                | `handl-plugin-bytes`               | `handl-plugin-token-bytes`                |
| `plugin-null`                | `plugin-token-null`                 | `handl-plugin-null`                | `handl-plugin-token-null`                 |
| `plugin-identifiers`         | `plugin-token-identifier`           | `handl-plugin-identifiers`         | `handl-plugin-token-identifier`           |
| `plugin-keywords`            | `plugin-token-keyword`              | `handl-plugin-keywords`            | `handl-plugin-token-keyword`              |
| `plugin-operators`           | `plugin-token-operator`             | `handl-plugin-operators`           | `handl-plugin-token-operator`             |
| `plugin-delimiters`          | `plugin-token-delimiter`            | `handl-plugin-delimiters`          | `handl-plugin-token-delimiter`            |
| `plugin-comment-tokens`      | `plugin-token-comment`              | `handl-plugin-comment-tokens`      | `handl-plugin-token-comment`              |
| `plugin-primary-arrays`      | `plugin-grammar-primary-array`      | `handl-plugin-primary-arrays`      | `handl-plugin-grammar-primary-array`      |
| `plugin-primary-identifiers` | `plugin-grammar-primary-identifier` | `handl-plugin-primary-identifiers` | `handl-plugin-grammar-primary-identifier` |
| `plugin-primary-objects`     | `plugin-grammar-primary-object`     | `handl-plugin-primary-objects`     | `handl-plugin-grammar-primary-object`     |
| `plugin-primary-values`      | `plugin-grammar-primary-value`      | `handl-plugin-primary-values`      | `handl-plugin-grammar-primary-value`      |
| `plugin-postfix-calls`       | `plugin-grammar-postfix-call`       | `handl-plugin-postfix-calls`       | `handl-plugin-grammar-postfix-call`       |
| `plugin-postfix-index`       | `plugin-grammar-postfix-index`      | `handl-plugin-postfix-index`       | `handl-plugin-grammar-postfix-index`      |
| `plugin-postfix-field`       | `plugin-grammar-postfix-field`      | `handl-plugin-postfix-field`       | `handl-plugin-grammar-postfix-field`      |
| `plugin-infix-pipe`          | `plugin-grammar-infix-pipe`         | `handl-plugin-infix-pipe`          | `handl-plugin-grammar-infix-pipe`         |
| `plugin-infix-assign`        | `plugin-grammar-infix-assign`       | `handl-plugin-infix-assign`        | `handl-plugin-grammar-infix-assign`       |
| `plugin-stmt-let`            | `plugin-grammar-stmt-let`           | `handl-plugin-stmt-let`            | `handl-plugin-grammar-stmt-let`           |
| `plugin-stmt-const`          | `plugin-grammar-stmt-const`         | `handl-plugin-stmt-const`          | `handl-plugin-grammar-stmt-const`         |
| `plugin-stmt-import`         | `plugin-grammar-stmt-import`        | `handl-plugin-stmt-import`         | `handl-plugin-grammar-stmt-import`        |
| `plugin-fmt-statements`      | `plugin-fmt-stmt`                   | `handl-plugin-fmt-statements`      | `handl-plugin-fmt-stmt`                   |
| `plugin-vm-rust`             | `plugin-target-rust`                | `handl-plugin-vm-rust`             | `handl-plugin-target-rust`                |
| `plugin-native-example`      | `plugin-target-native-example`      | `native-plugin-example`            | `handl-plugin-target-native-example`      |

Crates that already follow the convention, such as `plugin-ast-call`,
`plugin-grammar-call`, `plugin-token-number`, `plugin-fmt-expression`,
`plugin-diag-expression`, `plugin-lsp-completion`, and
`plugin-typecheck-core`, keep their current form.

The overlapping comment/doc-comment plugins
(`plugin-comments`, `plugin-doc-comments`, `plugin-token-comment`,
`plugin-comment-tokens`) were disambiguated: the canonical comment token
piece lives in `plugin-token-comment`, and the no-op
`plugin-comments` and `plugin-doc-comments` crates were removed.

## Consequences

### Positive

- `[workspace] members` can be enumerated mechanically from
  `crates/plugin-*`.
- Plugins sort cleanly by domain, making it easy to find all token,
  grammar, fmt, or target crates.
- Kit manifests and tooling can assume a uniform naming scheme.
- Duplicate and overlapping crates become obvious and can be merged.

### Negative

- Requires a bulk rename of many directories and package names.
- Every `Cargo.toml` dependency, kit manifest, and documentation
  reference must be updated.
- Short-term churn while the workspace is in transition.

## Migration plan

1. Produce a script or codemod that renames directories and updates
   `package.name` in each `Cargo.toml`.
2. Update the root `Cargo.toml` to add every `crates/plugin-*`
   directory to `[workspace] members`.
3. Reconcile `[workspace.dependencies]` so each plugin is declared once
   under its canonical `handl-plugin-<domain>-<construct>` name.
4. Resolve near-duplicates explicitly (delete or merge).
5. Update `kit-canonical` and any other kit manifests that list plugin
   ids.
6. Update `docs/`, `README`, and other references.
7. Validate the workspace with `cargo check` in CI.

## Implementation status

Implemented. The bulk rename in `crates/` and the root `Cargo.toml` was completed. The overlapping `plugin-comments` and `plugin-doc-comments` crates were removed; the canonical comment token remains `plugin-token-comment`, and no remaining grammar/AST/fmt/diag rules needed to be moved.

## Alternatives considered

- **Leave current names and only add missing members**: rejected because
  the existing inconsistencies continue to make discovery and
  automation difficult.
- **Drop the `plugin-` prefix from package names**, e.g.
  `handl-token-number`: rejected because it removes the explicit plugin
  identity and collides with base crates like `handl-ast` and
  `handl-lexer`.
- **Use more top-level domains such as `primary`, `postfix`, `infix`,
  and `stmt`**: rejected because these are grammar shapes, not primary
  domains, and would proliferate namespaces.
- **Keep `plugin-vm-*` for backends**: rejected because `target` is the
  architectural term already used for target plugins and keeps codegen
  distinct from the VM core.

## Related work

- ADR-0001: Generic Node, Trait-Based Capabilities, and Plugin-Driven
  Canonical Kit
- ADR-0002: Single `Plugin` Model for Host, Operations, Libraries,
  Targets, and Options
- `docs/explanations/design-principles.md`
- `kit-canonical` crate manifest
