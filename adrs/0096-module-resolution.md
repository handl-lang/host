---
title: Module Resolution — Path-Based Imports + `.handl.toml` Path Dependencies
type: adr
date: 2026-09-08
status: accepted
status_note: 'Accepted (amended: unified import model)'
---


# ADR-0096: Module Resolution — Path-Based Imports + `.handl.toml` Path Dependencies

## Decision


Module resolution is **path-based**, with a dependency table as the only
indirection. Given `import "<spec>"`:

1. **Dependency specifier.** If the first `/`-separated segment of `<spec>`
   names an entry in the project's `.handl.toml` `[dependencies]` table,
   the remainder resolves inside that package's directory:
   `import "arrows/fn/map.hnl"` → `<deps["arrows"]>/fn/map.hnl`.
   Dependency names shadow same-named directories relative to the module
   root.
2. **Relative path.** Otherwise `<spec>` resolves against the directory of
   the module currently being loaded (nested imports), falling back to the
   configured module root, then the process working directory.

`[dependencies]` maps an import name to a package directory, in either
form:

```toml
[dependencies]
arrows = "vendor/arrows"          # bare path
utils  = { path = "../shared" }   # table form (room for future specifiers)
```

Paths are relative to the manifest directory (absolute paths honored
as-is). The project manifest is discovered by walking up from the entry
file's directory (`Kit::discover` / `Kit::discover_with_eval`); the
manifest's directory becomes the module root. With no manifest, the entry
directory is the module root and there are no dependencies.

Only **vendored / path dependencies** are supported — no registry, no
network, no version resolution. Registry and content-addressed stores
remain the ADR-0010 design and can layer a `spec`-to-directory resolution
step in front of this mechanism without changing import semantics.

### Arrow-value sealing

Exported arrow values carry `body` indices into the _module's_ AST. Before
bindings cross into the importer's scope, `EvalEngine::seal_module_value`
(a defaulted trait hook — identity for engines whose values carry no
ambient-eval references) lets the tree-walk engine register the module's
eval frame (`register_home_frame`) and stamp un-homed arrows with
`__home__`, the same provenance mechanism `HandlFunction` already uses.


## Why

The `import` op exists in all three eval engines and `handl-runtime` owns a `ModuleLoader` (`load_module`/`resolve_import`), but two pieces were missing: a stated resolution policy for the `import "path"` specifier, and a way for a project to name external packages (`.handl.toml` had no `[dependencies]` section). ADR-0010 sketches the long-term package-manager design (content-addressed graph stores, registries, lockfiles) — it is a large design not needed to unblock local development.

## Not doing

- none recorded — legacy record; the design doc lists what was deferred.

## Consequences

- `import "arrows/fn/map.hnl"` works end-to-end via `hnl run`/`hnl eval`
- A module file is a single top-level statement (Pratt convention); its
- `from "<name>" import` forms still name _modules_ resolved via the
- The `loading` cycle-detector and the nested-import directory stack are

## Enforcement

- Code and tests are named in the design doc.

## Design

The full record (context, tables, phases, trade-offs): [`design/0096-module-resolution.md`](../design/0096-module-resolution.md).
