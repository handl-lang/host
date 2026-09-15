# ADR-0096: Module Resolution — Path-Based Imports + `.handl.toml` Path Dependencies

## Context

The `import` op exists in all three eval engines and `handl-runtime` owns a
`ModuleLoader` (`load_module`/`resolve_import`), but two pieces were
missing: a stated resolution policy for the `import "path"` specifier, and
a way for a project to name external packages (`.handl.toml` had no
`[dependencies]` section). ADR-0010 sketches the long-term package-manager
design (content-addressed graph stores, registries, lockfiles) — it is a
large design not needed to unblock local development.

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

## Unified import model (amendment)

There are two specifier domains sharing one statement family (`stmt.import`):

### Canonical surface forms

All of these parse under the canonical Pratt grammar and lower to a
`stmt.import` node carrying `payload.path`, `payload.names` (`"*"` or a
single export name), and `payload.alias`:

```handl
import "<spec>"                       -- splice all exports into scope
from "<spec>" import *                -- same, explicit
from "<spec>" import * as <alias>     -- bind the module record to <alias>
from "<spec>" import <name>           -- bind <name> to the module's export
from "<spec>" import <name> as <alias> -- bind <alias> to the export
```

The `as` tail is gated on the `as` keyword matching
(`TokenStep::requires_prev_match`) so a bare `from "p" import name`
followed by another statement never swallows that statement's leading
identifier.

For a **path** `<spec>` (anything that resolves to a file per the Decision
rules — `[dependencies]` first segment, module dir, module root, CWD) the
op is executable in all three eval engines: `prim_import` projects a named
export (erroring on a missing name), binds the module record under `* as`,
or splices exports. Named imports (`from "p" import name`) are the
canonical replacement for the old `import name from "p"` order.

### Module-name imports

A `<spec>` that is a **bare host-module name** — a single
`[A-Za-z_][A-Za-z0-9_-]*` segment that does not resolve to a file —
denotes a module on the host module chain (`str`, `fn`, `dedent`, ...),
not a path. Resolution:

- In `.hnl` sources loaded as `HandlModule`s, the declarations are already
  semantic: `analyze_source` parses them and `HandlModule::with_resolver`
  auto-wires the named modules as bridge-runtime dependencies.
- In whole-program hosts (`hnl run`, `hnl test`) the kit's module chain
  already supplies the bindings (qualified `str.trim` and bare `reverse`
  both resolve), so the declaration lines are **decorative and stripped
  textually** — `host-cli::strip_import_declarations` drops `from` lines
  whose specifier does not resolve to a file plus all `import <name> ...`
  lines; the test runner skips them unconditionally. (`hnl eval` evaluates
  a single expression, where in-expression `stmt.import` nodes execute
  normally.)
- In file-loaded modules, `ModuleLoader::strip_module_decl_lines` applies
  the same rule before lexing, so a `.hnl` module file may carry
  `from "str" import * as str` headers without failing as a missing file.

**Deferred:** a `resolve_import` fallback that resolves bare names against
the module registry (returning a module-namespace value) is future work,
pending `loader.hnl` becoming the installed default transform (the
`HandlLoader`/`install_loader` seam). The value model does not currently
expose a host module as a first-class record, so declarations suffice.

_Loader status update:_ the parsed-AST cache in `HandlFunction::call`
(thread-local `AST_CACHE` + `EvalEngine::eval_shared` /
`Runtime::eval_shared_with_globals`) removed the per-call re-parse that
originally blocked the default install — measured ~7× faster on
`loader.strip_decls` over `dedent.hnl` and ~19× on warm `str.*` calls.
The remaining blocker is the interpreted `str.*`/`char.*` body eval over
every input character (~43s release for a 22k-char transform), so
`loader.hnl` stays opt-in; see AGENTS.md "Loader self-hosting" for the
measured numbers and the `plugin-lib-handl/examples/loader_bench.rs`
harness.

### Static checking

`hnl typecheck` scopes the bound names of every `stmt.import` — the child
`var` (`import name`), `payload.alias`, and `payload.names` — both at
module level and inside `do { ... }` blocks, where statements bind
sequentially (an `assign` target or import is in scope for following
siblings). `from "<bare-name>" import *` scopes the module name itself so
`str.trim` is not an unbound wire. Splice forms with a path specifier
(`import "p"`, `from "p" import *`) cannot be statically enumerated;
names they bind are still flagged by the bootstrap checker — use the
`* as` or named form for statically-checked code.

### Arrow-module dialect

`plugin-ext-refinement` arrow modules keep their own surface syntax —
`import <name>[, <name>...] from "path"`, `import "path" as alias`,
`export ... from` — scanned by `arrow_parser`'s line scanner before arrow
expressions are parsed. This is **dialect-fixed**, not migrated: the whole
file format (arrow graphs, `rec { ... } ( ... )`, `~>`) is the dialect;
the only in-tree uses are `handl-packages/collection/sort.hnl` and the
`research/arrow-calculus-kernel-poc` corpus, exercised by refinement
tests. Note the canonical Pratt parser accepts `import x from "p"` as
`import (x)` and silently drops the trailing `from "p"` tokens (the parser
does not check cursor exhaustion) — the bootstrap typechecker's decl
collection reads the var child, which coincidentally matches the dialect's
bound name, so `hnl typecheck`/`lint` accept such lines while `hnl run`
strips them.

## Consequences

- `import "arrows/fn/map.hnl"` works end-to-end via `hnl run`/`hnl eval`
  and `Kit::discover`; covered by `kit-canonical/tests/import_export.rs`,
  which also covers `from "p" import *` / `* as` / `name` / `name as`
  forms and file-loaded modules carrying host-module headers.
- A module file is a single top-level statement (Pratt convention); its
  export is the root node's binding target (`x :: expr` → `x`). Multi-
  statement modules use a `do { ... }` root, which exports nothing.
- `from "<name>" import` forms still name _modules_ resolved via the
  `HandlModule`/`ModuleResolver` seam, not files — `hnl run` strips them
  when the specifier does not resolve to a file. Closing that gap
  (module-name fallback in `resolve_import`) is future work.
- The `loading` cycle-detector and the nested-import directory stack are
  cleaned up on error paths.

## Status

Accepted (amended: unified import model)
