# The SDK generator (sdkgen)

`sdkgen` is the plugin domain that turns a foreign API description — a C
header, a WebIDL spec, a tool's `--help` tree — into Handl port/adapter
packages (ADR-0107). This directory documents the framework itself: the
IR, the traits, the target registry, and the CLI. Per-backend reference
lives in the target repos; usage walkthroughs live in their READMEs.

- [The `hnl sdkgen` command](./cli.md) — flags, `--from-url`, exit codes.
- [Adding a target](./adding-a-target.md) — new repo, `register()`, the
  host feature line.
- [Generated-artifact conventions](./conventions.md) — port/adapter
  layout, descriptor vocabulary, doc layering.

## Crates

| Crate | Repo | Role |
| --- | --- | --- |
| `handl-plugin-sdkgen` | `host` (`crates/plugin-sdkgen`) | `SdkgenModule` IR, `SdkgenBackend`/`Emitter` traits, `SdkgenRegistry`, `SdkgenRequest`, `DocSource`, `http_get` |
| `handl-plugin-ext-sdkgen` | `host` (`crates/plugin-ext-sdkgen`) | `sdkgen_command` — argv → `SdkgenRequest` → `registry.run` |
| `hnl-ext-sdkgen` | `ext-sdkgen` | ADR-0149 cdylib packaging the command for the dumb `hnl` host |
| `handl-plugin-sdkgen-backend-c` | `sdkgen-c` | C header backend (libclang) |
| `handl-plugin-sdkgen-backend-webidl` | `sdkgen-webidl` | WebIDL backend |
| `handl-plugin-sdkgen-cli` | `sdkgen-cli` | CLI backend + `hnl-cli` emitter |
| `handl-plugin-sdkgen-emitter-file` | `sdkgen-file` | `hnl-file` emitter — `.hnl` port/adapter files |
| `handl-plugin-sdkgen-emitter-python` | `sdkgen-python` | `python` emitter — ctypes projection, plus the spec-input path |

## The pipeline

```text
foreign description (header / *.idl / *.clispec.yaml / tool name)
       │
       ▼
SdkgenBackend::parse_header(path) ──────────────► SdkgenModule   (IR)
       │                                          records, typedefs,
       │                                          functions, constants,
       │                                          imports, diagnostics
       ▼
[optional docs pass on the target record]        module.source = url
       │
       ▼
Emitter::emit(module, output) ──────────────────► <pkg>/<module>.hnl        (port)
                                                  <pkg>-<backend>-<slug>/   (adapter)
                                                    <module>.hnl
```

Three axes stay independent (ADR-0107 §3.1): the **backend** (what is
being parsed), the **target** (the ABI/convention the adapter must
satisfy), and the **foreign client** (`module.client_package` — the
runtime primitive family the adapter calls: `ffi`, `js`, `cli`).

## `SdkgenModule` — the IR

The IR is backend-agnostic data (`crates/plugin-sdkgen/src/lib.rs`).
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
| `diagnostics` | Non-fatal backend notes; `hnl sdkgen` prints them, emitters may stamp them |

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

```rust
trait SdkgenBackend { fn parse_header(&self, path: &Path) -> Result<SdkgenModule, SdkgenError>; }
trait Emitter       { fn emit(&self, module: &SdkgenModule, output: &Path) -> Result<(), SdkgenError>; }
```

Backends and emitters are registered under **names**, and a *target* is
a data record composing them (`crates/plugin-sdkgen/src/targets.rs`):

```rust
SdkgenTarget {
    name: "python",                 // what follows `hnl sdkgen`
    backend: "c",                   // resolved through the registry
    emitter: "python",              // resolved through the registry
    source_filters: true,           // accept --include/--exclude
    docs: None,                     // optional post-parse doc pass
    spec_input: Some(...),          // optional non-backend input path
}
```

`SdkgenRegistry::run(name, &SdkgenRequest)`:

1. Rejects `--include`/`--exclude` when `source_filters` is `false`
   (usage error, exit 2).
2. Runs `spec_input` when it matches `req.spec` (bypasses the pipeline;
   the `python` target uses this for yaml/json specs).
3. `backend.make(req)` → `parse_header(req.spec)` → stamps
   `module.source` → optional `docs` pass → prints `module.diagnostics`.
4. `emitter.make(req)` → `emit(module, req.output)`.

A missing backend/emitter id is a *link-time* error message
("needs backend `x`, which is not linked in"), not a compile-time dep —
the `python` target composes backend `c` without `sdkgen-python` ever
crate-depending on `sdkgen-c`.

`SdkgenRequest` carries everything factories need from argv: `spec`,
`source_url`, `output`, `target`, `include`/`exclude`, `config`, and
`lang` — the canonical `Language` injected by the host so emitters can
canonicalize emitted source without linking `handl-kit-data-prelude`
(the ADR-0128 "0.8 seam").

## Doc layering

`pick_doc(slot, overrides, keys, source, external_key)` resolves every
doc slot by the same precedence: `docs:` config override → comment
captured in the spec/header → external `DocSource` (`lookup(key)`).
`http_get(url, timeout_ms)` is the shared blocking GET used by doc
sources (man7.org, MDN) and by `--from-url`.

## Per-target docs

| Target | Backend repo doc | Emitter |
| --- | --- | --- |
| `c` | [sdkgen-c](https://github.com/handl-lang/sdkgen-c/blob/main/docs/index.md) | `hnl-file` ([sdkgen-file](https://github.com/handl-lang/sdkgen-file/blob/main/docs/index.md)) |
| `webidl` | [sdkgen-webidl](https://github.com/handl-lang/sdkgen-webidl/blob/main/docs/index.md) | `hnl-file` |
| `python` | backend `c` | `python` ([sdkgen-python](https://github.com/handl-lang/sdkgen-python/blob/main/docs/index.md)) |
| `cli` | [sdkgen-cli](https://github.com/handl-lang/sdkgen-cli/blob/main/docs/clispec.md) | `hnl-cli` ([emission](https://github.com/handl-lang/sdkgen-cli/blob/main/docs/emission.md)) |
