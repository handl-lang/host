+++
id = "P-004"
title = "kit system: kit.hnl manifests, use-case kits, lib-* gap analysis"
status = "open"
created = "2026-11-22"
priority = "high"
+++

# kit system: kit.hnl manifests, use-case kits, lib-* gap analysis

Status: **draft** · Created: 2026-11-22

## Goal & Overview

`kit-*` = curated sets of `lib-*` modules (+ plugins) for a use-case —
manifests, not code. Existing precedent: `kit-data-prelude/data/kits/*.yaml`
already implements this (`extends` chains, `modules:`, `caps:`,
`base_modules:`) inside a Rust repo; P-004 makes it the public contract.

## Decisions (user, 2026-11-22)

1. **Manifest format**: `kit.hnl` — a `.hnl` data literal, consistent with
   `pkg.hnl`.
2. **Repo shape**: `kit-*/` repo containing `kit.hnl` (+ optional glue `.hnl`).
3. **No umbrella prelude kit** — kits must be distinctive per use-case;
   environment-dependent composition stays out.
4. Kits are named by use-case. Approved names so far:
   - `kit-test` — test runner curation
   - `kit-web-server` — HTTP server curation
   - `kit-web-browser-iterop` — browser/client-side JS interop (was
     `kit-web-client-js`)
   - `kit-web-worker-iterop` — web worker interop
   - `kit-cli-app` — CLI application curation
   - NOT for now: `kit-data`, `kit-native`

## Manifest sketch — kit.hnl v1 (user-approved 2026-11-22)

```hnl
{
  name: "web-server",
  description: "serve HTTP from handl",
  license: "MIT",
  libs: ["serve-epoll", "http-route", "http-rfc", "json", "posix-c"],
}
```

**v1 = pure curation, nothing else.** No `plugins` field (native needs are
implied transitively by lib deps — `ffi.call`/cylib covers all native
access; host statically links the Rust plugin layer anyway). No `entry`/
glue field (rejected as coupled/non-pluggable — kit-specific logic must
live in a `lib-*` so kits stay swappable manifests; serve loops etc.
graduate to libs like `lib-http-route`). `kit.hnl` = `{ name,
description, license, libs }` only. Open: whether `libs` entries are pkg
names or `lib-*` repo names; loader mechanism (host-side vs pkg).

## Kit repo reclassification (from triage)

Already true kits (data-only, stay): `kit-data-prelude` (manifest exemplar —
likely becomes the canonical kit registry or splits), `kit-prelude-lang`,
`kit-eqsat`.

Impl repos needing rehoming:
- `kit-core` → host/platform machinery
- `kit-prelude` → composition root (host-*/tool-*); ships `handl-kit-mlopt` +
  `hnl-ext-mlopt` → move to mlopt-product/ext
- `kit-stdlib`, `kit-server`, `kit-test`, `kit-ffi`, `kit-lang` → module
  bundles with single consumer (kit-prelude); merge or rename to reflect
  content
- `kit-pkg` → tool-*/host-* (project discovery for ext-*)

## Kit designs (investigated 2026-11-22)

Naming rule confirmed: `lib-{port}` = contract, `lib-{port}-{impl}` = backend.

### kit-web-browser-iterop

```hnl
{ name: "kit-web-browser-iterop",
  libs: ["js", "fetch-js", "ws-whatwg-wasm", "json"],
  plugins: ["compile-js"] }
```

`lib-js` = the JS-interop contract (`call`/`invoke`/`await`/`error`, `JS` grade);
`lib-fetch-js` = fetch over generated WebIDL + `js.await` (the precedent);
`tool-compile-js` already targets `browser` (esm/iife/inline).
**New libs:** DOM bindings live **inside `lib-js`** (user decision
2026-11-22 — no separate repo): specs already exist in
`tool-sdkgen-webidl/specs/` (`dom.idl`, `html.idl`, `websockets.idl`,
`storage.idl`, `console.idl`, …); generated package lands in
`lib-js/handl-packages/generated/dom-webidl-browser/` via sdkgen-webidl,
consumed via dep/path imports (lib-fetch-js pattern). Optional
`lib-js-host` marshalling shim; `lib-event-dom` later. **Blocker:**
no JS→handl callback mechanism —
`lib-js` has call/invoke/await but no `on_event` registration; DOM listeners
and worker `onmessage` need a host injection or JS-side trampoline.

### kit-web-worker-iterop

```hnl
{ name: "kit-web-worker-iterop",
  libs: ["js", "worker"],
  plugins: ["compile-js"],
  glue: "src/worker-run.hnl" }
```

Mostly greenfield: `lib-concurrent` task model + `lib-js.await` exist, but no
Worker binding/postMessage/structuredClone anywhere. **New libs:**
Worker bindings via WebIDL → `tool-sdkgen-webidl` → generated package inside
`lib-js` (lib-dom pattern — user decision: no `lib-worker` repo; worker is a
kit-curated surface, not a lib). `DedicatedWorkerGlobalScope`/`Worker` specs +
`postMessage`/`onmessage`/`terminate`; optional `lib-concurrent-js` backend
implementing `concurrent.Run` over MessageChannel/setTimeout.

### kit-cli-app

```hnl
{ name: "kit-cli-app",
  libs: ["args", "stdio", "posix-c", "cli", "json", "str-handl", "collections", "time-c"],
  glue: "src/main.hnl" }
```

**Critical gap:** nothing reads own-process argv or stdio today — `lib-cli` is
child-process spawn only; `lib-fs` is a libpython hack (broken); `lib-posix-c`
has env/path/time but no argv/stdio. **New libs:** `lib-args` (pure parser
taking argv as param — spec reuses `cli.FlagSpec` shape; argv sourced via
`/proc/self/cmdline` or runner injection `{"args":…}` protocol),
`lib-stdio` (`print`/`eprint`/`read_line`/`read_all`/`exit` over libc
`write`/`read`/`exit` ffi.call — descriptor vocabulary proven in cli-c),
`lib-log` (optional, pure over stdio+time), real `lib-fs` rewrite.

### kit-test

`lib-test` ships six pure-.hnl modules (`test`/`property`/`benchmark`/`fuzz`/
`snapshot`/`assert`) — but note: these are the **in-language** test surface;
the host `hnl test` runner (tool-test) is a separate mechanism. kit-test
curates the former.

```hnl
{ name: "kit-test",
  libs: ["test", "rand-stub", "serialize", "combinators", "type", "tag"] }
```

(`rand-stub` binds as module `rand` — reconcile pkg.hnl naming first.)
Current Rust `kit-test` crate = delivery binding only (vendors lib-test +
lib-rand-stub); the kit.hnl replaces that role.
**Gaps:** no coverage lib, no mock lib, no reporter lib; `property`/`fuzz`
generate only `seeded_float` inputs; `lib-test/pkg.hnl` modules list stale.

### kit-web-server

The server already exists: `lib-serve` (contract) + `lib-serve-epoll`
(implementation — accept loop, keep-alive, pipelining, chunked req bodies,
TLS via opts.tls, WebSocket upgrade via opts.on_ws). Handler = `req ~> resp`
arrow. Linux-only (epoll/libcurl/libssl).

```hnl
{ name: "kit-web-server",
  libs: ["serve-epoll", "concurrent-epoll", "concurrent", "http-rfc",
         "tls-libssl", "ws-frame", "ws-frame-handl", "ws-whatwg-c",
         "ws-c", "json", "str", "bytes", "curl", "libc"],
  glue: ["glue/routes.hnl"] }   -- optional router/static conveniences
```

Serve call needs no glue: `serve((req) ~> resp, {port: 8080, tls?, on_ws?})`.
**Gaps:** no router lib (by design routing is in-handler — `lib-http-route`
would be the addition), no multipart, no static files, no chunked responses,
no HTTP/2, TLS/WS pull a heavy dep tree (consider plain vs tls/ws kit split).
NOTE: `lib-serve-epoll` modules aren't registered in any Rust kit crate yet —
kit.hnl may be the first-class resolution mechanism for them.

### lib-* gap list (consolidated)

New libs needed: ~~`lib-worker`~~ (WebIDL-generated pkg inside lib-js instead), `lib-mri` (was lib-args — DONE), `lib-stdio` (DONE: contract + stdio-c), `lib-log`
(optional), `lib-concurrent-js` (optional), `lib-http-route` (optional),
`lib-event` finish or `lib-event-dom`. Planned backends: `lib-json-c`,
`lib-json-js` (generated pkg inside lib-js, lib-dom pattern), `lib-fs-js`
(Node fs API is the de-facto standard). **OS surface migrates to posix
naming** (user decision 2026-11-22, T-117): `lib-posix` contract (POSIX.1 —
fs/fd/env/path/**time**; clock fns are POSIX so time folds in — DONE:
`lib-time`/`lib-time-c` folded into the posix family, `lib-posix-c`
implements `now_ms`/`instant_ns`/`elapsed_ns`/`epoch` with a direct
`clock_gettime` `ffi.call` (no `time-c` dep), `lib-posix-stub` covers the
same names with zeros, consumers swept (kit-prelude `time_c.rs`→
`posix_time.rs`, kit-test `benchmark` `time.`→`posix.` + posix-stub
backend registered as `posix`, host stdlib `clock_gettime` module repointed
to `lib-posix-c/src/clock_gettime.hnl`); local `lib-time`/`lib-time-c`
clones retired) + `lib-posix-c` impl (libc ffi.call — absorbs
lib-posix-c env/path/time AND the lib-fs rewrite) + `lib-posix-stub`. Full
rename incl. module names (`posix-c`→`posix-c`, `posix-stub`→`posix-stub`);
consumer sweep required. `lib-posix-c`/`lib-posix-stub`/`lib-fs` repos retire.
`lib-json-lib`
deleted
(libpython hack; lib-json + lib-json-handl suffice). Stubs: `lib-math-js` +
`lib-rand-js` implementing (js.call); `lib-{str-c,str-js}` pending user
decision. Optional later: `lib-coverage`, `lib-mock`, reporter lib,
`lib-http-multipart`, HTTP/2.

## lib-* gap analysis

## Requirements

- Systematic: every kit answers "what can you do with handl after loading
  this?" — no umbrella kits.
- New `lib-*` designed to same `pkg.hnl` rules; native backends via
  `ffi.call`/sdkgen per P-003.
- Migration of impl repos sequenced after manifest format is frozen.
