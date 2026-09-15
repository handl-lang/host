# ADR-0100: LSP as Language Data

## Context

ADR-0044 ("language as data") established that engines and tooling are generic
and data-driven: a new language is a YAML fragment merged into a `Language`,
never an engine recompile. The formatter, linter, and typechecker already
follow this. The LSP support did not.

Currently the canonical LSP surface is spread across four small crates
(`plugin-lsp-completion`, `plugin-lsp-goto`, `plugin-lsp-hover`,
`plugin-lsp-symbol`) and each one hard-codes handl constructs: `prev.text ==
"from"`, `prev.text == "import"`, scanning for `axiom` / `law` / `export`,
`.hnl` extension handling, `---` and `--` doc comments, and the `time/time.hnl`
module-directory layout. Renaming a keyword or adding a new export form requires
editing Rust in 3–4 places. This is the exact pattern ADR-0044 bans.

## Decision

LSP behavior is first-class `Language` data, with a single generic engine crate
and a thin handl adapter.

1. **`LspDef` fragment** (`handl-language::fragments::lsp`), merged into a new
   `Language::lsp: FragmentSet<LspDef>` (added with `serde(default)` so
   pre-ADR-0100 serialized languages still load). The canonical data lives in
   `crates/handl-kit-data-canonical/data/lsp.yaml` as a list of fragments.

   An `LspDef` has:

   - `rules` — a registry of named rule _recipes_ (`id -> LspRule`).
   - `lsp` — a map from LSP method id (e.g. `textDocument/documentSymbol`)
     to an ordered list of rule ids.

   Each `LspRule` has:

   - `kind` — the `plugin-lsp` provider that evaluates it.
   - `match` — an optional `LspMatch` condition (AST `!Match`, token `!Token`,
     or engine `!Native`).
   - `config` — provider-specific data captured as a free `serde_json::Map`.
     No `Language` schema change is needed to add a new `kind`.

   This makes the LSP surface _method-keyed_, not section-keyed. A language can
   enable only the LSP methods it wants, in any combination, with whatever rule
   recipe mix it needs. Missing methods return empty or `null` responses.

2. **Two new crates** replace the four `plugin-lsp-*` crates:

   - `crates/plugin-lsp` — the generic LSP engine. It receives an LSP method
     string, looks up the method's rule list in `Language::lsp`, and evaluates
     each rule by dispatching on its `kind`. It is not handl-specific.
   - `crates/plugin-lsp-std` — the canonical handl adapter. It registers the
     `handl-kit-data-canonical` `Language` with `plugin-lsp` and provides the
     `LspPlugin` wiring that `host-lsp` loads.

   The old `plugin-lsp-completion`, `plugin-lsp-goto`, `plugin-lsp-hover`, and
   `plugin-lsp-symbol` crates are deleted and removed from the workspace.

3. **Validation and resilience.** `merge_fragments` loads `lsp.yaml` and merges
   it into `Language::lsp`. A new `validate_lsp` pass (analogous to
   `validate_transforms` for lint rules) will check rule ids, token ids, AST
   kinds, roles, payload paths, `TransformCondition` references, and method
   `kind` handlers at `Language` build time. Missing or unknown roles, tokens,
   node kinds, or rule references are caught when the `Language` is assembled,
   not at LSP request time.

4. **Fallbacks and robustness.** Each export rule can declare `fallback:
source` for signatures (use the node's own source text when a dedicated
   signature role is absent) and `dotted_last_segment: true`/`false` for names.
   The engine silently skips a rule if the matched node does not provide the
   required role, so one malformed construct does not break the whole outline
   or completion list.

5. **Caching is the host's responsibility.** `plugin-lsp` is stateless and
   operates on a `(source, Language)` snapshot. `host-lsp` already re-parses on
   every change; `plugin-lsp` may cache per-document exported-name tables if
   profiling shows it is necessary, but the initial implementation does not.

6. **Everything is optional.** The `lsp` fragment itself is optional; a
   `Language` without one returns empty or `null` LSP responses. Each LSP
   method is optional; `textDocument/hover` can be omitted while
   `textDocument/completion` is kept. Inside a rule, `match`, `docs`,
   `signature`, and `dotted_last_segment` are all optional. A language with no
   comments can omit `docs`; one with no imports can omit `import_path` and
   `import_name` rules. `plugin-lsp` ignores missing data and never assumes a
   construct exists unless the language declares it.

## Consequences

- Adding a new handl export form (e.g. `op` or `handler`) is an `lsp.yaml` edit
  plus the corresponding `ast.yaml` shape — no `plugin-lsp` recompile.
- Changing the `from`/`import` surface is a token or grammar data change;
  `plugin-lsp` does not hard-code any token text.
- A future `plugin-lsp-python` or `plugin-lsp-foo` can reuse the same engine
  with a different `Language` and `lsp.yaml`; the engine is generic over the
  `handl-language` model.
- `!Match` is shared vocabulary with `lints.yaml` and `transforms.yaml`, so
  validation, condition evaluation, and third-party fragment authoring are
  consistent.
- A third-party kit can contribute only the LSP methods it wants by including
  only those keys in its `lsp` map. It can add new `kind` values once
  `plugin-lsp` learns them, without changing the `Language` schema.
- The generic engine is larger and more abstract than the current hand-coded
  plugins. It must be tested with a minimal `Language` and `lsp.yaml` in
  `plugin-lsp/tests`.
- `plugin-lsp-std` should eventually contain no logic: it is only a
  registration crate. Any handl-specific code that cannot be expressed in
  `lsp.yaml` is a defect in the data model, not a reason to add Rust to
  `plugin-lsp-std`.

## Related work

- Supersedes `project/adrs/archived/ADR-LSP.md` (separate `plugin-lsp-*` plugins)
  and `project/adrs/ADR-LSP-remaining.md` (hover/source-position plugins).
  ADR-0100 keeps the thin `host-lsp` dispatcher but makes the plugins generic
  and data-driven rather than handl-specific.
- Builds on ADR-0044 (language as data), ADR-0081 (AST child roles), and
  ADR-0097 (lint rules as data). The `lsp.yaml` fragment uses the same
  `!Match` vocabulary and the same `Language` fragment mechanics.
- Does not replace ADR-0096 (runtime module resolution). The `lsp.yaml`
  `import_path.module_layouts` and `import_name.module_layouts` are editor-side
  path completion patterns only; the runtime resolver is unchanged. A future
  ADR may unify the two if the editor and runtime can share a single
  module-resolution data fragment.

## Status

accepted
