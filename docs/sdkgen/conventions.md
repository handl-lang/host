# Generated-artifact conventions

## Port and adapter packages

Every `.hnl`-emitting run produces a **port/adapter pair** (ADR-0107):

```text
<output>/<module>.hnl                       — port: abstract `export` types/arrows
<output>/../<pkg>-<backend>-<slug>/<module>.hnl
                                            — adapter: concrete `export … :=` impls
```

- `<pkg>` is the basename of `--output` (e.g. `--output …/generated/gh`
  → port package `gh`).
- `<backend>` is `module.backend_id` (`c`, `webidl`, `cli`); `hnl-cli`
  uses `cli` for its adapter slug — `gh-cli-host`.
- `<slug>` is `target_slug(module.target)`: `{os}-{arch}` taken from
  triple positions (`x86_64-unknown-linux-gnu` → `linux-x86_64`);
  single-word targets (`host`, `browser`) pass through verbatim.
- Header stamps: `--- Generated port for <pkg>.<module>`,
  `--- <client> target: <target>`, `--- source: <url>` when
  `--from-url` was used, `--- diagnostic:`/`--- confidence:`/`---
  warning:` lines for backend diagnostics the emitter chooses to
  surface.

Emitted `.hnl` is run through `format_source_with_language` with the
injected `Language`, so generated files satisfy `hnl fmt --check`; on a
format error the raw text is still written. Spec-supplied argument names
that collide with canonical Handl keywords get a `_` suffix
(`data` → `data_`).

## Port shape

- Records → `export Name := { field: Port, … }`.
- Typedefs → `export Name := <port>`.
- Constants → `export NAME: Int` (value bound in the *adapter* —
  constants are target-specific data).
- Functions → `export Port: (Args) ~[1]~> Result<T, E>` — `E` is
  `Errno` (ffi client) or `JsError` (js client); the `cli` client emits
  `Result<Output, Spawn>`. The `~[1]~>` grade marks the arrow
  effectful; adapter bodies are plain `~>`.

Functions whose marshalling has independent out channels get declared
result shapes on the port: `{Port}Result` (`{ ret, buf, outs, vals }`
subset) and `{Port}Msg` for inline `in_struct`/`io_struct` overrides.

## Adapter shape

Adapters call the client package's runtime primitives:

| Client | Call | Error channel |
| --- | --- | --- |
| `ffi` | `ffi.call(lib, sym, [args], [descriptors], ret, false)` | `ffi.errno()` when `errno_guard` |
| `js` | `call(library, method, [args], [js_descriptors], ret, false)` | `error()` — `null` on success |
| `cli` | `run(argv(prog, subpath, positionals, flag_specs, opts, rest))` | `Result<Output, Spawn>` from `cli-c` |

The `ffi` client is reached by qualified call (`ffi.call`) not
`from "ffi" import { call }` — the package's `call` export is a type
signature, not an arrow. `js` imports `call`/`error` normally. `cli`
adapters import `{ run, argv }` from `cli-c`.

## Descriptor vocabulary

`Function.arg_descriptors` are strings the emitter passes to the runtime
marshaller. Each backend defines its own subset; the union so far:

**C / ffi** (`sdkgen-c`, defined by `plugin-ffi`'s marshaller):

| Descriptor | Port type | Consumes a Handl arg? | Meaning |
| --- | --- | --- | --- |
| `i8`…`i64`, `u8`…`u64`, `isize`, `usize`, `size_t`, `ssize_t`, `f32`, `f64` | `Int`/`Nat`/`Float` | yes | scalar slot (arg slots widen sub-32-bit ints to `i32`) |
| `cstr` | `String` | yes | NUL-terminated `char *` |
| `cbytes` | `Bytes` | yes (2 C args) | `const char *` + length pair |
| `in_buf` | `Bytes` | yes (2 C args) | `const void *`/record `*` + length pair |
| `out_buf:<n>` | `Bytes` (or `{Port}Result.buf`) | no (2 C args: ptr, capacity) | callee-filled buffer; lone `out_buf` unwraps `ret` as the byte count |
| `out_ptr:<n>` | via `{Port}Result.outs` | no | out-pointer buffer |
| `io_buf:<n>` | via `{Port}Result.outs` | no (2 C args: ptr, len cell) | in-out buffer — capacity in, actual length out |
| `io_scalar:<t>` | `Int` arg + `{Port}Result.vals` | yes | value-result scalar — caller seeds the cell, C writes back |
| `in_ptr` | `Bytes` | yes | raw pointer from a bytes buffer |
| `strv` | `[String]` | yes | NUL-terminated `char **` argv vector |
| `in_struct:{…}` | the record (or `{Port}Msg` on override) | yes | `const struct T *` — marshaller builds the struct |
| `out_struct:{…}` | `{Port}Result.buf` | no | `struct T *` out-param, `ffi.call` allocates |
| `io_struct:{…}` | `{Port}Msg` | yes | in-out struct |
| `rec:<name>` / `rec:{…}` | record / `Obj` | — | nested record field inside a struct descriptor |
| `arr:<n>:<elem>` | `Bytes` (`:u8`) / `[Int]` | — | inline `T[n]` field |
| `ptr` | `Handle`/`Int` | yes | opaque pointer pass-through |
| `handle` | `Handle` | yes | resource-table handle |
| `void` / `()` | `()` | no | `void` return/argument |

Struct descriptors carry real layout: `name:kind@offset[:size]` with
`@`/`:` suffixes where a bare kind can't imply width (`ptr`, `arr:`,
`rec:`).

**WebIDL / js** (`sdkgen-webidl` → `hnl-file` `js_descriptor`):

`void`→`js_void`, `bool`→`js_boolean`, all numeric →`js_number`,
`promise[:T]`→`js_promise`; backend-emitted `js_string`, `js_bytes`,
`js_object` pass through. `js_object` is the catch-all for `any`,
`object`, `Promise`, `sequence`, `record`, nullable, union, and named
foreign types (all port as `Handle`).

**CLI** (`sdkgen-cli` `pos:`/`opt:`/`rest:`/`meta:` — see
[sdkgen-cli/docs/emission.md](https://github.com/handl-lang/sdkgen-cli/blob/main/docs/emission.md)):

`pos:<name>` positional in argv order · `opt:<kind>:<name>:<flag>` named
option (`flag`|`string`|`int`|`strings`) · `rest:<name>` variadic tail ·
`meta:<hint>` emitter hints (e.g. `meta:json-capable`), ignored when
rebuilding argv.

`meta:json-capable` is rendered on the adapter binding as a doc comment
pointing at the free-function composition —
`parse(tag.untag(r).value.stdout)` with `parse` from lib-json
(`from "json" import { parse }`) — never a helper. The rejected shape
was a method adapter (`x.json()`): not composable, not treeshakeable,
and it would hide the
pure `~>` boundary. Same story for line-oriented output: `lines` from
`str-handl`. `Output.stdout` is already a `String`, so nothing needs a
`text()`-style accessor.

Descriptors that consume no Handl arg (`out_buf:*`, `out_ptr:*`,
`io_buf:*`, `out_struct:*`, `io_struct:*`, `void`) expand to
callee-allocated slots; `port_args` lists only the consuming ones.
`io_scalar:*` is the exception among the `io_*` family — the caller
seeds the cell, so it does consume an arg, with the written-back value
surfacing in `vals`.

## Where output lives

`handl-packages/generated/` is the conventional output root. Two repo
conventions coexist:

- **`lib-*` repos keep specs and tests, not output.** `generated/` is
  gitignored and regenerated by the gate — `lib-cli-gen` runs `hnl
  sdkgen cli specs/<tool>.clispec.yaml` per spec in
  `scripts/hnl-gate.sh` on every CI run. Committed: `specs/` +
  `src/*.test.hnl` smoke tests.
- **Backend repos may commit output** when the generated package is
  itself the product: `sdkgen-webidl` tracks
  `handl-packages/generated/` (389 files) so the webref-pinned SDKs are
  reviewable like any other source (ADR-0107: "generated output is
  committed").

The host repo gitignores fetched spec inputs (`generated/*.webidl`,
`*.idl`) and `.mdn-cache.json` — `generate` artifacts, not source.

## Doc layering

`pick_doc` precedence, per slot: `docs:` config override → comment
captured from the source → external `DocSource`. `external_docs: false`
in the config disables the external layer. `c` resolves through man
pages (local files → `man` → man7.org); `webidl` through MDN with a
persistent `.mdn-cache.json` beside the output dir.
