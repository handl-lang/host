# ADR-0149: `host-cli` is a dumb host; `ext-*` extensions load dynamically

- Status: Accepted
- Date: 2026-10-17
- Repos: `host-cli` (renamed from `cli`), `host-lsp` (renamed from `lsp`), `ext-*` (new), `platform`, `kernel`

## Context

`host-cli` hardcodes every tool (`run`, `eval`, `fmt`, `lint`, `typecheck`,
`test`, `compile`, `sdkgen`, …) as a static dispatch arm, and each tool's
wiring crate (`plugin-ext-*`) lives in `platform`. Adding a tool means
editing the host's dispatch and rebuilding the host. That couples the CLI
surface to the platform's release cadence and contradicts the org's own
host/wire model: the host should provide capabilities; plugins should
consume them — direction the CLI itself never honored.

## Decision

1. **The host knows nothing about tools.** `host-cli` exposes exactly one
   mechanism: discover extensions, read their manifest, register the
   commands they declare, dispatch argv to them. No `match` arm per tool;
   there are no "built-in" tools.
2. **Extensions are dynamic.** An extension is a shared library
   (cdylib) — `libhnl_ext_<name>.{so,dylib,dll}` — discovered at runtime,
   never a cargo dependency of the host. `cargo build` of `host-cli`
   produces a host that can only report its loaded extension set.
3. **The ABI is JSON over a C boundary.** Three exported symbols per
   extension:
   - `handl_ext_manifest() -> char*` — `{"name","version","commands":[{name,usage}]}`
   - `handl_ext_dispatch(cmd: *const c_char, input: *const c_char) -> char*` —
     `input` = `{"args":[..], "cwd":.., "env":{..}}`; returns
     `{"code":int, "stdout":.., "stderr":..}`.
   - `handl_ext_free(char*)` — extension-owned strings freed by the
     extension that allocated them.
   JSON keeps the boundary free of Rust-type ABI fragility
   (`TaggedValue`/`serde_json` shapes stay inside each side). Marshalling
   reuses the conventions in `plugin-ffi`.
4. **One repo per extension** (`ext-fmt`, `ext-lint`, `ext-typecheck`,
   `ext-eval`, `ext-test`, `ext-compile`, `ext-sdkgen`, …), same ADR-0128
   recipe as `lib-*`: crate `hnl-ext-<name>` (cdylib + rlib), git dep
   `branch = "main"`, local `[patch]` dev-link. `plugin-ext-diag` stays in
   `platform` — it is shared diagnostic plumbing, not a tool.
5. **Discovery**: `--ext-dir <dir>` flag, else `$HANDL_EXT_DIR`, else
   `~/.handl/ext/`; dev builds additionally scan `<exe-dir>/ext/` and the
   workspace `target/debug/` for `libhnl_ext_*`. Loading is eager at
   startup (manifest read only — no tool code runs until its command is
   dispatched).

## Consequences

- Tooling ships on its own release cadence; the host pins nothing.
- The `Extension`/`Plugin` substrate stays where it is — extensions are
  *hosts of plugins*, and internally keep using the same
  `DefaultHost`/`CapabilityRequest` machinery; only the packaging changes.
- `ext-*` crates still compile to `rlib` for tests — unit tests link
  statically; only the shipped artifact is dynamic.
- Repo renames `cli`→`host-cli`, `lsp`→`host-lsp` make the host/tool
  split legible in the org.
- Deliberately out of scope: wasm-component extensions and self-hosted
  `.hnl` extensions — both need RFC-0001's story; the JSON ABI chosen here
  does not preclude either.
