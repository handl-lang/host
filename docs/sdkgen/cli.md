# `hnl sdkgen` — the CLI surface

`hnl sdkgen` is an `hnl-ext-sdkgen` cdylib command (ADR-0149): the dumb
host discovers `libhnl_ext_sdkgen` via `--ext-dir`, `$HANDL_EXT_DIR`,
`~/.handl/ext/`, or a `target/debug/` next to the `hnl` binary. In-repo
the semantics live in `handl-plugin-ext-sdkgen::sdkgen_command(lang,
args)` (`crates/plugin-ext-sdkgen/src/lib.rs`) — the cdylib resolves the
project (or canonical) kit and injects its `Language` as `req.lang`, so
emitted `.hnl` is canonicalized against the same language the host
typechecks.

## Synopsis

```text
hnl sdkgen <target> [<header|spec> | --from-url <url>] --output <dir>
           [--target <triple>] [--include <glob>] [--exclude <glob>]
           [--config <yaml>]
```

`<target>` is looked up in the `SdkgenRegistry`; the usage line lists
the registered names (`c|webidl|python|cli` with default features).

| Flag | Effect |
| --- | --- |
| positional | Local spec/header path, or — for `cli` — a bare tool name to probe. Mutually exclusive with `--from-url`. |
| `--output <dir>` | Required. The port package dir; the adapter lands in the sibling `<pkg>-<backend>-<slug>` dir. The basename of `--output` is the port package name. |
| `--target <triple>` | Backend target selector. `c`: libclang `-target` triple (default: host triple). `webidl`: any slug (default `browser`). `cli`: adapter-package slug (default `host`). |
| `--include <glob>` | Comma-separated globs; only accepted on targets with `source_filters` (`c`, `python`). Unioned with config `headers:` entries. |
| `--exclude <glob>` | Same shape; a match vetoes even an include match. |
| `--config <yaml>` | Backend-specific overlay file (`*.sdkgen.yaml`). Loaded inside the backend's `make`, so each backend defines its own schema. |
| `--from-url <url>` | Fetch the spec over HTTP (`http_get`, 30 s) into the dir next to `--output`, under the URL basename — the `generated/` spec convention, so `module.library` still derives from the upstream file stem. The URL is stamped as `module.source` and emitted as `--- source:` lines. |

## Exit codes

`SdkgenTargetError` maps to the process exit code:

| Code | Meaning |
| --- | --- |
| `0` | Success |
| `1` | Generation failure — config/backend/parse/emit errors (`SdkgenTargetError::Failed`), including a missing linked component |
| `2` | Usage — no args, unknown target or flag, missing `--output`/input, `<header|spec>` + `--from-url` together, `--include`/`--exclude` on a `source_filters: false` target (`SdkgenTargetError::Usage`) |

Diagnostics in `module.diagnostics` are non-fatal: they print as
`sdkgen: diagnostic: …` on stderr and the run continues.

## Feature wiring

`handl-plugin-ext-sdkgen` gates each target behind a Cargo feature
(ADR-0128 §"Registration: link-time first"):

```toml
[features]
default = ["c", "webidl", "python", "cli"]
c       = ["dep:handl-plugin-sdkgen-backend-c", "dep:handl-plugin-sdkgen-emitter-file"]
webidl  = ["dep:handl-plugin-sdkgen-backend-webidl", "dep:handl-plugin-sdkgen-emitter-file"]
python  = ["c", "dep:handl-plugin-sdkgen-emitter-python"]
cli     = ["dep:handl-plugin-sdkgen-cli"]
```

`python` implies `c` because its target record names backend `c` —
resolved through the registry, not a crate dep. `sdkgen c` needs
libclang at runtime; set `LIBCLANG_PATH` when it is not on the loader
path.
