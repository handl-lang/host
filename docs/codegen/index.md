# The codegen framework

`hnl codegen` is the plugin domain that turns a foreign API description —
a C header, a WebIDL spec, a tool's `--help` tree — into Handl
port/adapter packages (ADR-0107), reconciled through the
`hnl-engine-codegen` desired-state kernel (P-001 / PL-004). This
directory documents the framework itself: the IR, the traits, the
target registry, and the CLI. Per-backend reference lives in the target
repos; usage walkthroughs live in their READMEs.

The pre-PL-004 name `sdkgen` survives only as the *domain/IR* name —
[`SdkgenModule`], the `"sdkgen"` IR kind, and backend ids (`c`,
`webidl`, `cli`). The command surface, crates, and repos are `codegen`.

- [`hnl codegen`](./cli.md) — verbs (`apply`/`plan`/`destroy`/`revert`),
  flags, `--from-url`, `--check`, exit codes.
- [Adding a target](./adding-a-target.md) — new repo, `register()`, the
  host feature line.
- [Generated-artifact conventions](./conventions.md) — port/adapter
  layout, descriptor vocabulary, doc layering.

## Crates

| Crate | Repo | Role |
| --- | --- | --- |
| `handl-plugin-codegen` | `host` (`crates/plugin-codegen`) | `SdkgenModule`/`SpecModule` IR, `CodegenRegistry`, `CodegenRequest`, `DocSource`, `http_get`, kernel re-exports |
| `handl-plugin-ext-codegen` | `host` (`crates/plugin-ext-codegen`) | `codegen_command` — argv → `CodegenRequest` → `registry.run`/`plan`/`destroy`/`revert` |
| `hnl-ext-codegen` | `ext-codegen` | ADR-0149 cdylib packaging the command for the dumb `hnl` host |
| `hnl-engine-codegen` | `engine-codegen` | The lifecycle kernel: `IrModule`/`Source`/`Emitter` traits, `Change` sets, `ArtifactStore`/`FsStore`, `Engine` plan/apply/destroy/revert, manifest + journal |
| `handl-plugin-codegen-backend-c` | `engine-codegen-c` | C header backend (`Source<SdkgenModule>`, libclang) |
| `handl-plugin-codegen-backend-webidl` | `engine-codegen-webidl` | WebIDL backend |
| `handl-plugin-codegen-cli` | `engine-codegen-cli` | CLI backend + `hnl-cli` emitter |
| `handl-plugin-codegen-emitter-file` | `engine-codegen-file` | `hnl-file` emitter — `.hnl` port/adapter files |
| `handl-plugin-codegen-emitter-python` | `engine-codegen-python` | `python` emitter — ctypes projection, plus the spec-input path |
| `hnl-engine-fmt` | `engine-fmt` | The `fmt` target — `FmtSource`/`FmtEmitter` over the token-stream `FmtModule` IR (PL-004 T-090) |

## The pipeline

```text
foreign description (header / *.idl / *.clispec.yaml / tool name)
       │
       ▼
Source::parse ────────────────────────────────► SdkgenModule   (IR)
       │                                          records, typedefs,
       │                                          functions, constants,
       │                                          imports, diagnostics
       ▼
[optional docs pass on the target record]        module.source = url
       │
       ▼
Emitter::materialize ─────────────────────────► Vec<Change>    (desired state)
       │
       ▼
Emitter::verify ──────────────────────────────► honest-decline gate
       │
       ▼
Engine::apply over FsStore ───────────────────► <pkg>/<module>.hnl        (port)
       (manifest + journal under                  <pkg>-<backend>-<slug>/   (adapter)
        .handl/codegen/)                           <module>.hnl
```

Three axes stay independent (ADR-0107 §3.1): the **backend** (what is
being parsed — now a `Source` impl), the **target** (the ABI/convention
the adapter must satisfy), and the **foreign client**
(`module.client_package` — the runtime primitive family the adapter
calls: `ffi`, `js`, `cli`).

Emitters are **pure**: `materialize` renders a desired [`Change`] set
and never touches the filesystem. `Engine::apply` is the only writer —
it diffs desired vs. current (`plan`), writes atomically, and records
a manifest + append-only journal under `.handl/codegen/` so runs are
drift-checked and `revert`-able.

## `SdkgenModule` — the IR

The IR is backend-agnostic data (`crates/plugin-codegen/src/lib.rs`).
Emitters may rely only on these fields — backend vocabularies enter
through `imports` and the descriptor strings, never through new IR
variants.

| Field | Meaning |
| --- | --- |
| `client_package` | Runtime client the adapter calls (`ffi`, `js`, `cli`) |
| `backend_id` | Backend id, used to name the adapter package |
| `target` | Target selector (triple for `c`, `browser`, `host`, …) |
| `library` | Foreign library the adapter binds (`libc.so.6`, a JS module name, a tool name; empty lets emitters default it) |
| `module` | Module/file stem of the emitted port |
| `records` | `Record { name, fields, size, align, doc }` — struct/dictionary-like declarations |
| `functions` | `Function` — see below |
| `typedefs` | `Typedef { name, port, descriptor, doc }` — port aliases |
| `enum_constants` | `EnumConst { name, value: i64, doc }` — integer constants |
| `imports` | `ModuleImport { package, names }` — extra `from "pkg" import {…}` lines the port needs, declared as data so emitters stay backend-free |
| `source` | Provenance stamp (set from `--from-url`); emitters render `--- source:` |
| `per_record_files` | Layout hint: one `<Record>.hnl` pair per record plus `types.hnl` (used by `webidl`; `c` leaves it off because its records cross-reference by name) |
| `spec_dump` | Resolved-spec dump requested via `--emit-spec`; the registry surfaces it as a request-level side effect |
| `diagnostics` | Non-fatal backend notes; `hnl codegen` prints them, emitters may stamp them |

`Function` is the workhorse:

| Field | Meaning |
| --- | --- |
| `name` | Foreign symbol / emitted adapter arrow name |
| `port_name` | `UpperCamelCase` name of the `export` arrow type in the port |
| `port_args` | `Arg { name, port }` — the Handl-visible parameters only |
| `arg_descriptors` | One marshalling descriptor per C-level argument (see [conventions](./conventions.md)); descriptors like `out_buf:` consume no port arg |
| `return_port` / `ret_descriptor` | Port return type and its descriptor |
| `out_struct` | Record name when a `struct T*` out-param was auto-mapped |
| `variadic` | Function is variadic (C `...`, variadic rest args) |
| `errno_guard` | Whether emitters wrap the call in `ret < 0 → err(errno())`; `false` for out-of-band error channels (`UErrorCode*`, `GetLastError`, `cli`) |
| `c_decl` | The foreign declaration, verbatim-ish (C signature, `Interface.member`, literal argv) — documentation and, for `js`, the call path |
| `doc` | Doc comment captured at parse time |

## Traits and the registry

A target composes a [`Source<I>`] (lift: spec → IR) and an
[`Emitter<I>`] (render: IR → desired `Change` set) — paired at compile
time via [`CodegenRegistry::register_typed`], or by name through the
registry records (`crates/plugin-codegen/src/registry.rs`):

```rust
CodegenTarget {
    name: "python",                 // what follows `hnl codegen`
    source: "c",                    // resolved through the registry
    emitter: "python",              // resolved through the registry
    source_filters: true,           // accept --include/--exclude
    docs: None,                     // optional post-parse doc pass
    spec_input: Some(...),          // optional non-backend input path
}
```

[`CodegenRegistry::run`] (the `apply` verb) and
[`CodegenRegistry::plan`] (the `plan`/`--check` verb — same pipeline,
no mutation):

1. Rejects `--include`/`--exclude` when `source_filters` is `false`
   (usage error, exit 2).
2. Runs `spec_input` when it matches `req.spec` on `apply` only
   (transitional direct-emit path; `plan`/`materialize` reject it).
3. `source.make(req)` → `Source::parse` → stamps `module.source` →
   optional `docs` pass → `--emit-spec` dump → `module.diagnostics`.
4. `emitter.make(req)` → `Emitter::materialize(module, ctx)` →
   `Emitter::verify` (the honest-decline gate).
5. `apply` only: `Engine::apply` reconciles the `Change` set against
   the store rooted at `req.output` — manifest + journal under
   `.handl/codegen/`. `destroy` deletes exactly the artifacts the
   prior manifest records; `revert` undoes a journal run.

Sources and emitters are stored **type-erased** (`AnySource` /
`AnyEmitter`) so one registry serves every IR family (`SdkgenModule`,
`SpecModule`, `FmtModule`, …); pairing is compile-time via
`register_typed`, name-based records resolve ids at run time.

A missing source/emitter id is a *link-time* error message ("needs
source `x`, which is not linked in"), not a compile-time dep — the
`python` target composes source `c` without `engine-codegen-python`
ever crate-depending on `engine-codegen-c`.

`CodegenRequest` carries everything factories need from argv: `spec`,
`source_url`, `output`, `target`, `include`/`exclude`, `config`,
`emit_spec`, `cache_dir`, and `lang` — the canonical `Language`
injected by the host so emitters can canonicalize emitted source
without linking `handl-kit-data-prelude` (the ADR-0128 "0.8 seam").
`Source`/`Emitter` impls never see `Language` inside `EmitContext`;
they capture `Arc<Language>` at `make` time.

## Doc layering

`pick_doc(slot, overrides, keys, source, external_key)` resolves every
doc slot by the same precedence: `docs:` config override → comment
captured in the spec/header → external `DocSource` (`lookup(key)`).
`http_get(url, timeout_ms)` is the shared blocking GET used by doc
sources (man7.org, MDN) and by `--from-url`.

## Per-target docs

| Target | Backend repo doc | Emitter |
| --- | --- | --- |
| `c` | [engine-codegen-c](https://github.com/handl-lang/engine-codegen-c/blob/main/docs/index.md) | `hnl-file` ([engine-codegen-file](https://github.com/handl-lang/engine-codegen-file/blob/main/docs/index.md)) |
| `webidl` | [engine-codegen-webidl](https://github.com/handl-lang/engine-codegen-webidl/blob/main/docs/index.md) | `hnl-file` |
| `python` | source `c` | `python` ([engine-codegen-python](https://github.com/handl-lang/engine-codegen-python/blob/main/docs/index.md)) |
| `cli` | [engine-codegen-cli](https://github.com/handl-lang/engine-codegen-cli/blob/main/docs/clispec.md) | `hnl-cli` ([emission](https://github.com/handl-lang/engine-codegen-cli/blob/main/docs/emission.md)) |
| `fmt` | — (token-stream IR, no foreign spec) | `fmt` ([engine-fmt](https://github.com/handl-lang/engine-fmt)) |
