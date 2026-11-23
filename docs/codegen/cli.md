# `hnl codegen` — the CLI surface

`hnl codegen` is an `hnl-ext-codegen` cdylib command (ADR-0149): the
dumb host discovers `libhnl_ext_codegen` via `--ext-dir`,
`$HANDL_EXT_DIR`, `~/.handl/ext/`, or a `target/debug/` next to the
`hnl` binary. In-repo the semantics live in
`handl-plugin-ext-codegen::codegen_command(lang, args)`
(`crates/plugin-ext-codegen/src/lib.rs`) — the cdylib resolves the
project (or canonical) kit and injects its `Language` as `req.lang`, so
emitted `.hnl` is canonicalized against the same language the host
typechecks.

## Synopsis

```text
hnl codegen [plan|apply|destroy|revert] <target> [<spec> | --from-url <url>] --output <dir>
           [--target <triple>] [--include <glob>] [--exclude <glob>]
           [--config <yaml>] [--emit-spec <path|->] [--cache-dir <dir>]
           [--check|--dry-run|--plan] [--run <id>]
```

The leading verb defaults to `apply` (the pre-PL-004 `sdkgen`
behaviour). `<target>` is looked up in the `CodegenRegistry`; the usage
line lists the registered names (`c|webidl|python|cli|fmt` with default
features).

| Verb / flag | Effect |
| --- | --- |
| `apply` (default) | Full pipeline: parse → docs → materialize → verify → `Engine::apply`. Writes are journaled under `<output>/.handl/codegen/` (manifest + append-only journal), drift-checked against `before_hash`, atomic per artifact. |
| `plan` | Same pipeline through `Emitter::verify`, then `Engine::plan` — prints the desired-vs-store diff (`+` create / `~` update / `-` delete / ` ` unchanged). **No mutation.** |
| `--check` / `--dry-run` / `--plan` | `apply`-mode aliases for `plan`. `--check` additionally exits **1 when the plan has pending mutations** — the CI gate. |
| `destroy` | Deletes exactly the artifacts the target's prior manifest records (plus the manifest itself). Manifest-driven: unrecorded files are never touched; no drift check. |
| `revert [--run <id>]` | Undoes a journal run's before-images. Default run: the one that produced the target's current manifest; `--run` selects an explicit journal id. |
| positional | Local spec/header path (or, for `cli`, a bare tool name to probe; for `fmt`, the `.hnl` file to format). Mutually exclusive with `--from-url`. `destroy`/`revert` take no positional spec. |
| `--output <dir>` | Required for `apply`/`plan` (the `FsStore` root); also required-but-unused-by-pipeline for `destroy`/`revert` (the store root the manifest/journal live under). For spec-emitting targets it is the port package dir; the adapter lands in the sibling `<pkg>-<backend>-<slug>` dir. The basename of `--output` is the port package name. For `fmt`, pass the file's directory so the artifact ref resolves in place. |
| `--target <triple>` | Backend target selector. `c`: libclang `-target` triple (default: host triple). `webidl`: any slug (default `browser`). `cli`: adapter-package slug (default `host`). |
| `--include <glob>` | Comma-separated globs; only accepted on targets with `source_filters` (`c`, `python`). Unioned with config `headers:` entries. |
| `--exclude <glob>` | Same shape; a match vetoes even an include match. |
| `--config <yaml>` | Backend-specific overlay file (`*.sdkgen.yaml` — the filename convention kept its name). Loaded inside the source's `make`, so each backend defines its own schema. |
| `--emit-spec <path\|->` | Backend hint delivered on `req.emit_spec`; the `cli` backend folds it into its config's `emit_spec` post-load (the flag wins). `-` writes the resolved spec to stdout — the dump is the only stdout output; diagnostics stay on stderr. |
| `--cache-dir <dir>` | Doc/spec cache root (e.g. webidl's MDN `.mdn-cache.json`) — deliberately outside the output tree. |
| `--source-url <url>` | Provenance only: stamp `--- source:` without fetching. |
| `--from-url <url>` | Fetch the spec over HTTP (`http_get`, 30 s) into the dir next to `--output`, under the URL basename — the `generated/` spec convention, so `module.library` still derives from the upstream file stem. The URL is stamped as `module.source` and emitted as `--- source:` lines. |
| `--run <id>` | `revert` only: select a journal run explicitly. |

## `hnl codegen fmt` — file→file formatting (T-090)

The `fmt` target formats a `.hnl` file in place through the same
lifecycle: `hnl codegen fmt <file.hnl> --output <its-dir>` (or any
ancestor dir — the artifact ref is the spec path relative to the store
root). `plan`/`--check` diffs formatted-vs-current bytes with no
mutation; `apply` writes the canonical text via `Engine::apply`. This
is the desired-state counterpart of `hnl fmt` (which stays on the
`FmtPlugin` capability seam in `ext-fmt`).

## Exit codes

`CodegenTargetError` maps to the process exit code:

| Code | Meaning |
| --- | --- |
| `0` | Success — including `--check` on a clean plan |
| `1` | Failure — config/source/parse/materialize/verify/apply errors (`CodegenTargetError::Failed`), a missing linked component, an incomplete `ApplyReport`, a failed `destroy`/`revert` entry, **or `--check` finding pending plan mutations** |
| `2` | Usage — no args, unknown verb/target/flag, missing `--output`/input, `<spec>` + `--from-url` together, `--include`/`--exclude` on a `source_filters: false` target, store-only flags on `apply`/`plan` or vice versa (`CodegenTargetError::Usage`) |

Diagnostics in `module.diagnostics` are non-fatal: they print as
`codegen: diagnostic: …` on stderr and the run continues.

## Feature wiring

`handl-plugin-ext-codegen` gates each target behind a Cargo feature
(ADR-0128 §"Registration: link-time first"):

```toml
[features]
default = ["c", "webidl", "python", "cli", "fmt"]
c       = ["dep:handl-plugin-codegen-backend-c", "dep:handl-plugin-codegen-emitter-file"]
webidl  = ["dep:handl-plugin-codegen-backend-webidl", "dep:handl-plugin-codegen-emitter-file"]
python  = ["c", "dep:handl-plugin-codegen-emitter-python"]
cli     = ["dep:handl-plugin-codegen-cli"]
fmt     = ["dep:hnl-engine-fmt"]
```

`python` implies `c` because its target record names source `c` —
resolved through the registry, not a crate dep. `c` needs libclang at
runtime; set `LIBCLANG_PATH` when it is not on the loader path.
