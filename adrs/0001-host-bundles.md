---
title: Host Bundles
type: adr
date: 2026-08-24
status: accepted
status_note: 'Partially implemented. `SandboxHost` exists with `max_depth` and `max_calls`. Missing: memory budget, time budget, virtual-module opt-out.'
---

# Host Bundles

## Context

The handl VM is generic over a `Host` trait — the kernel defines no runtime, no standard library, no I/O. All functions, control flow, and capabilities come from the host. A host is a **bundle**: a composition of modules that determines what programs can do.

The same program runs under different hosts with different capabilities. The program declares its needs implicitly through the functions it calls; the host decides whether to fulfill them.

This ADR catalogs the host bundles we envision, their capability surfaces, and their relationships.

## Decision

Hosts are categorized by **what they provide** (module set) and **where they run** (execution target). All implement the same `Host` trait — the VM and language kernel do not change.

### Existing hosts

- **`DefaultHost`** — built-ins only: `add`, `mul`, `concat`, `len`, `var`, `resolve`, `apply`, `array`. Interface-agnostic base layer.
- **`SandboxHost`** — pure library modules + resource limits (`max_depth`, `max_calls`). No I/O, no `import()`. For untrusted code. Already implemented in `handl-host-sandbox`.
- **`TypeHost`** — type-level evaluation. Types are JSON values; `is_compatible` is an handl function. Meta-circular.
- **`MetaHost`** — reflection: `typeof`, `keys`, `eval`, `parse`, AST inspection.
- **`CacheHost`** — decorator that memoizes any inner host. Cross-machine resumability via `CallCache`.
- **`ResolverHost`** — decorator that adds runtime `import()` backed by a `ModuleResolver`.

### Planned hosts

#### `handl-host-sandbox`

Pure functions only. No I/O, no side effects, no `import()` at runtime (but opt-out from virtual-module system). Deliberately limited for untrusted code. Resource limits (depth, memory, time budget) enforced by the host. A host that provides only pure functions is automatically sandboxed — this is a policy decision, not a language feature.

#### `handl-host-native`

Full OS access. Composes `lib-platform-std` (`StdFs`, `StdEnv`, `StdPath`, `StdTime`, `StdHttpClient`, `StdCmd`, `StdTmp`) + `lib-utils` + `lib-crypto` + `lib-datetime` + `lib-rand` + `lib-intl` + `lib-kv`. The "batteries-included" host for running on a real machine. Long-term reference host. The `lib-platform` service traits already define the abstract interfaces — `handl-host-native` wires them to the stdlib implementations.

#### `handl-host-server`

Long-running server process. Async I/O, connection pooling, request lifecycle. Checkpointing for long computations via `Continuation`. Composes `ResolverHost` + `CacheHost` + `lib-platform-std` + custom network modules.

#### `handl-host-edge`

Edge/CDN runtime (Cloudflare Workers, V8 isolates). Cold-start optimized, limited memory, no persistent filesystem. Request/response oriented. Fast instantiation from serialized state.

#### `handl-host-wasm`

Runs in WebAssembly. Bridges to host environment via imported functions. No direct filesystem or process access — capabilities are delegated to the WASM runtime's host functions.

#### `handl-host-js`

Compiles to JavaScript. Provides `fetch`, `localStorage`, DOM queries, `console`, etc. No filesystem, no process spawning. The `Continuation` mechanism (serializable frame stack) enables SSR patterns: server yields on a `fetch` operation, ships the continuation to the browser, client resumes with a browser-side `fetch`. Uses `AsyncFunction` for network I/O.

#### `handl-host-web-worker`

Runs in a Web Worker. Offloads computation from the main thread. Post-message-based communication with the main thread. No DOM access. Can use `import()` via `ResolverHost` with an in-memory or fetch-based resolver. Suitable for parallel evaluation across workers.

#### `handl-host-cuda`

Parallel execution. Functions map to GPU kernels/shaders. Batch evaluation across arrays. Different execution model — the VM dispatches to GPU operations instead of sequential function calls.

#### `handl-host-game`

Game engine embedding. Functions for entity queries, physics, event dispatch, scene graph manipulation. Tight integration with a game loop. The host provides domain-specific functions; the language becomes a scripting layer.

#### `handl-host-embedded`

Minimal, constrained. For microcontrollers / embedded targets. Stripped module set, no heap allocation beyond what's needed, no `serde_json` (custom compact value representation). Maybe even no async. The ultimate "small footprint" host.

### Composition patterns (orthogonal decorators)

| Decorator              | Adds                            |
| ---------------------- | ------------------------------- |
| `CacheHost`            | Memoization layer               |
| `ResolverHost`         | Runtime `import()`              |
| `ModuleHost`           | Module composition              |
| Resource-limit wrapper | Depth / memory / time budgets   |
| Audit wrapper          | Call logging / observability    |
| Permission wrapper     | Capability-based access control |

### Distinguishing dimensions

| Dimension       | Range                                                       |
| --------------- | ----------------------------------------------------------- |
| I/O access      | none → filesystem → network → process → GPU                 |
| Execution model | sync → async → lazy → resumable → parallel                  |
| Sandboxing      | full OS → limited → pure only                               |
| Persistence     | stateless → cached → content-addressed → checkpointable     |
| Target          | native → browser/WASM → embedded → GPU → game engine → edge |
| Module set      | empty (`DefaultHost`) → utils → platform → everything       |

## Consequences

- **No host is special.** Every host is a bundle of modules implementing the same `Host` trait. The kernel never needs to know which host it's running under.
- **Capability is emergent.** A program that calls `fs.read` only works on hosts that provide a filesystem module. The program declares needs implicitly; the host fulfills or rejects them.
- **Sandboxing is a policy, not a feature.** A host with only pure functions is automatically sandboxed. No language-level sandboxing mechanism is needed.
- **Cross-target portability is real but constrained.** Programs using only pure functions port everywhere. Programs using platform-specific modules port only to hosts that provide those modules.
- **New hosts are cheap to build.** Implement `Host`, compose modules, done. No kernel changes, no language changes, no fork required.
- **Host proliferation is a risk.** Too many hosts with overlapping capability surfaces could fragment the ecosystem. Conventions for standard module sets per target should emerge.

## Status

Proposed
