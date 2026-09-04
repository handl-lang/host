# ADR-0077: Stdlib Migration to Handl — `op` Declarations, Axioms, and the Elimination of Per-Library Rust

## Status

Accepted. Builds on [ADR-0069 — Arrow-Native Modules](./0069-arrow-native-modules.md),
[ADR-0052 — Scheme IR Contracts](./0052-scheme-ir-contracts.md),
[ADR-0068 — Predicates as Types](./0068-predicates-as-types.md),
and [ADR-0051 — FFI Rust Binding Facade](./0051-ffi-rust-binding-facade.md).

## Context

### The current state

The `plugin-lib-*` family of crates (~60 crates) implements the handl
standard library. Each crate is Rust. They fall into two populations:

1. **Pure-logic libs** (`hello`, `stats`, `collections`, `utils`, `parse`,
   …) — the logic is already handl-authored in `.hnl` files under
   `handl-packages/`, but each crate carries ~400 lines of hand-rolled
   Rust bridge code (`bridge.rs`, `module.rs`, `plugin.rs`, `lib.rs`) that
   `include_str!`s the `.hnl` source, builds a cached per-thread `Runtime`,
   and evaluates `(source).func(args)` per call.

2. **Native-primitive libs** (`base64`, `crypto`, `fs`, `http`, `tls`,
   `os`, `rand`, …) — the logic _and_ the implementation are Rust, because
   they wrap Rust crates or syscalls. There is no handl source for them.

### What already exists

- **Eval-side module registry** (`plugin-module`): `Module` trait,
  `ModuleRegistry`, `ModuleLookup`, `ModuleRule`/`ModuleRuntime`. The
  tree-walk eval engine dispatches `ArrSource::Foreign(name)` to
  `ModuleLookup::lookup(name)` → Rust `Function::call(args)`.
- **Inference-side module loader** (`plugin-ext-refinement`):
  `ModuleLoader` (ADR-0069) loads `.hnl` files, parses `import`/`export`,
  produces `HashMap<String, Scheme>` (named arrow graphs) + cached
  contracts. All three normalization strategies (A: compile-time desugar,
  B: flatten, C: runtime lazy load) are implemented. This feeds
  refinement inference only — **not** eval.
- **Axiom system** (`plugin-ext-refinement`): `axiom name : T -> T,
guarantee: P` declarations in `.hnl` contract files. Loaded into
  `ContractRegistry`. Used for static refinement inference. **Not** wired
  into the eval-time foreign-call boundary.
- **`ArrSource::Foreign(name)`** in `Scheme` IR — the capability-tracked
  foreign-call marker. The tree-walk engine dispatches it to
  `ModuleLookup`. But the foreign name, its type, and its contract all
  live in Rust, not in handl source.

### The gap

The two module systems (eval-side registry, inference-side loader) are
not connected. There is no path from a `.hnl` file to a callable eval-time
`Module` without a per-library Rust bridge. And for native libs, there is
no handl surface syntax for declaring foreign bindings — the foreign
name, type, and contract are all Rust-authored.

A partial bridge (`plugin-lib-handl` with `HandlModule` /
`HandlLibraryPlugin`) was just built and proven by migrating
`plugin-lib-stats`. It eliminates the per-library bridge for pure-logic
libs. But it does not address native libs, axioms, or capability
declaration.

### Why axioms and proofs must live in handl

Rust offers no proof mechanism. The stdlib's correctness story —
refinement contracts, termination guarantees, compositional inference —
can only be expressed and verified in handl. Today the axioms for
irreducible Rust primitives live in `handl-packages/contracts/prim.hnl`,
but they are disconnected from the actual Rust `Function` impls they
describe, and they are not enforced at the eval boundary.

For the migration to be complete, every public arrow in the stdlib must
carry:

- A `---` doc comment (the public API contract in prose).
- An `axiom` declaration (the formal contract: input type, output type,
  `guarantee`, optional `requires`/`termination`).
- A proof (where the body is handl-authored — the proof is compositional
  via the contract cache, ADR-0069 §4).

For native libs, the Rust body cannot be proven. But the _axiom_ can be
stated in handl, and the Rust body is trusted to honor it. This is the
FFI boundary: handl declares and verifies the contract; Rust implements
it.

## Decision

Migrate the entire `plugin-lib-*` stdlib to handl-authored `.hnl` files,
eliminating per-library Rust crates. The migration proceeds in six
phases, in dependency order. Each phase is independently shippable.

### Required content of every migrated `.hnl` file

Every public arrow in the stdlib must carry:

1. **`---` doc comment** — the public API contract in prose, immediately
   above the arrow definition. This is the documentation surface (rendered
   by the doc generator, shown in LSP hover).

2. **`axiom` declaration** — the formal contract: input type, output
   type, `guarantee`, optional `requires`/`termination`. This is the
   verification surface (consumed by the inference engine, enforced at
   the eval boundary after Phase 3).

3. **Proof** — for handl-authored arrows, the proof is compositional via
   the contract cache (ADR-0069 §4): each arrow is inferred once, its
   contract cached, importers reuse it. No separate proof file is needed
   — the inference engine generates the proof from the arrow graph +
   imported contracts. For `op` arrows (Rust-backed), no proof is
   generated — the axiom is trusted (the Rust handl is assumed to honor
   it), but the contract is still enforced at the boundary (Phase 3).

Example of a complete migrated pure-logic arrow:

```handl
--- stats.sum — sum all numeric elements in an array.
--- Non-numeric elements are silently skipped.
--- Contract: SumOf(output, input)
sum = xs ~>
  loop { xs: xs, acc: 0 }(
    [xs, acc] ~> match xs {
      []: { out: acc },
      [h, ...t]: if type.type_of(h) == "num" then
        { state: { xs: t, acc: acc + h } }
      else
        { state: { xs: t, acc: acc } }
    }
  )
```

Example of a complete migrated native `op`:

```handl
--- base64.encode — encode a string to base64.
--- Contract: IsBase64(output)
op "base64.encode" : String -> String
  guarantee: IsBase64(output)

handl "base64.encode" = rust_base64_encode
```

### Phase 1 — Pure-logic libs via `HandlModule` (in progress)

Migrate all pure-logic libs (`hello`, `collections`, `utils`, `parse`,
…) to `.hnl` files loaded by the generic `plugin-lib-handl` crate
(`HandlModule` / `HandlLibraryPlugin`), following the proven
`plugin-lib-stats` migration.

Each `.hnl` file carries:

- `---` doc comments on every public arrow (the API contract in prose).
- The arrow logic (already handl-authored for hello/stats; port the rest
  from Rust).

The Rust crates collapse to: one `plugin-lib-handl` crate (already built)

- N `.hnl` files + a kit manifest entry per lib. No per-library Rust.

**Done when:** `plugin-lib-hello`, `plugin-lib-collections`,
`plugin-lib-utils`, `plugin-lib-parse` (and any other pure-logic libs)
are migrated; their Rust bridge code is deleted; all tests pass.

### Phase 2 — `op` declaration form (FFI)

Add surface syntax for declaring operations (Rust-backed functions) in
`.hnl` source, so native libs can declare their interface in handl
without a Rust `Module` struct. The declaration is an **operation** in
the algebraic-effects sense (Plotkin & Pretnar; Sanada's arrow calculus
with operations and handlers, ADR-0052 §"Arrow Handlers"); the Rust body
that implements it is the **handl** (the handler that interprets the
operation).

```handl
-- fs.hnl
op "fs.read" : Path -> Bytes
  requires: PathValid(path)
  guarantee: IsUtf8(output)

op "fs.write" : (Path × Bytes) -> Unit
  guarantee: True
```

The `handl` block binds a Rust function (the handler) to an `op`:

```handl
handl "fs.read" = rust_fs_read
handl "fs.write" = rust_fs_write
```

This requires:

- **Lexer/Pratt parser support** for `op` and `handl` declarations (new
  top-level forms, like `import`/`export` in ADR-0069). Keywords
  `token.keyword.op` and `token.keyword.handl` are already registered in
  `tokens.yaml` and `lexer_rules.yaml`.
- **An `OpDecl` node** in the parsed module, carrying the name, type
  signature, and optional axiom (requires/guarantee/termination).
- **A `HandlDecl` node** in the parsed module, carrying the op name and
  the Rust function identifier to bind.
- **A host-side registry** mapping declared `op` names → Rust `Function`
  impls (the handls). The Rust bodies stay; only the declaration moves
  to handl.
- **`HandlModule` integration** — when a `.hnl` file contains `op`
  declarations, the loaded module exposes them as callable functions that
  dispatch to the Rust registry (same `ArrSource::Foreign` path, but the
  name/type/contract now come from handl source, not Rust).

**Done when:** at least one native lib (e.g. `plugin-lib-base64`) is
migrated — its `Module`/`Function` trait impls deleted, replaced by an
`op` declaration in `.hnl` + a Rust `Function` (the handl) registered
under the declared name. All tests pass.

### Phase 3 — Wire axioms to eval

Attach `ContractRegistry` contracts to the `Foreign` dispatch boundary so
that declared `op`s carry enforced `guarantee`/`requires`, not just
inferred offline.

This requires:

- **Linking `OpDecl` axioms to `ContractRegistry`** — when a module
  with `op` declarations is loaded, its axioms are registered in the
  contract registry under the op name.
- **Eval-time checking** — the tree-walk engine, when dispatching
  `ArrSource::Foreign(name)`, consults the contract registry for `name`
  and checks the `requires` predicate against the call arguments (at
  runtime; static checking is the inference engine's job).
- **Proof obligation** — for handl-authored arrows, the proof is
  compositional (ADR-0069 §4 contract cache). For `op` arrows, the
  axiom is trusted (the Rust body is assumed to honor it); no proof is
  generated, but the contract is still enforced at the boundary.

**Done when:** an `op` with a `requires` predicate rejects a call
that violates it; an `op` with a `guarantee` is checked against the
return value (where feasible). Tests cover both.

### Phase 4 — Native libs migrated to handl interfaces

Migrate all native-primitive libs (`base64`, `crypto`, `fs`, `http`,
`tls`, `os`, `rand`, …) to `.hnl` interface files using the Phase 2
`op` form + Phase 3 axiom wiring.

Each native lib becomes:

- A `.hnl` file with `op` declarations + `---` doc comments + axioms.
- A Rust crate that registers `Function` impls (the handls) under the
  declared `op` names (the bodies — I/O, crypto, syscalls — stay Rust).
- No `Module`/`Plugin` trait scaffolding in Rust; the `HandlModule` /
  `HandlLibraryPlugin` from Phase 1 handles registration.

The Rust that remains is pure implementation: `fn call(&self, args) ->
Result<Value, EvalError>` bodies. No trait boilerplate, no bridge code,
no per-lib plugin scaffolding.

**Done when:** all native libs are migrated; their `Module`/`Plugin`
trait impls deleted; only the `Function` bodies remain in Rust.

### Phase 5 — Capability declaration in handl

Move `CapabilityDecl` from Rust `Language` construction into `.hnl`
source, so a handl library declares its own capability surface.

```handl
-- fs.hnl
cap "fs.read"
cap "fs.write"

op "fs.read" : Path -> Bytes
  ...
```

This requires:

- **Surface syntax** for `cap` declarations in `.hnl`.
- **Loader integration** — the module loader extracts `cap`
  declarations and feeds them into the `Language`'s capability set
  (currently built in Rust via `CapabilityDecl::new`).
- **`HandlLibraryPlugin` integration** — the plugin reads the declared
  capabilities and contributes them via `ContributionContext`.

**Done when:** a handl library's capability set is fully determined by
its `.hnl` source; no Rust `CapabilityDecl::new` calls remain for
migrated libs.

### Phase 6 — ADR-0069 Phase 7: arrows/ as importable modules

Move `handl-packages/` arrow files to be importable `.hnl` modules (ADR-0069
Phase 7). The loader already supports this; the eval wiring from Phase 1
makes the imported arrows callable. Verify the full cascade: atoms →
insert → sort → user programs, all via imports with compositional
inference, under each of the three normalization strategies.

**Done when:** `handl-packages/**/*.hnl` are importable; a user
program can `import sort from "collection/sort"` and call it; inference
composes across the import boundary.

### Multi-target support — interface/impl split

The stdlib is already platform-agnostic by design (ADR-0002). Native
libs follow an **interface/impl** split:

- **Interface crate** (`plugin-lib-X`) — abstract trait, no I/O deps,
  platform-neutral. Defines the API surface.
- **Impl crate** (`plugin-lib-X-std` / `-native` / `-wasm`) — concrete
  backend for a specific target. The host picks which impl to register.

Existing examples:

```
plugin-lib-os          → abstract traits (Fs, Env, Path, Time, Cmd, …)
plugin-lib-os-std      → impl via Rust stdlib (std::fs, std::process)

plugin-lib-http-client → abstract trait (HttpClient)
plugin-lib-http-client-std → impl via ureq

plugin-lib-ws-whatwg   → abstract
plugin-lib-ws-whatwg-native → impl via native syscalls
plugin-lib-ws-whatwg-wasm   → impl via WASM (cfg target_arch = "wasm32")
```

The `op`/`handl` migration preserves and strengthens this split:

- **Interface** → `.hnl` file with `op` declarations + axioms + `cap`
  declarations (platform-neutral, shared across all targets)
- **Impl** → Rust `handl` bodies registered under the `op` names (one
  per target: `std`, `native`, `wasm`, `js`, …)

```handl
-- os.hnl (platform-neutral, shared across all targets)
--- fs.read — read a file as bytes.
--- Contract: IsUtf8(output)
op "fs.read" : Path -> Bytes
  guarantee: IsUtf8(output)

cap "fs.read"
```

```rust
// os-std crate (Rust native)
handl "fs.read" = rust_std_fs_read   // std::fs::read

// os-wasm crate (WASM target)
handl "fs.read" = wasm_fs_read        // browser fetch API

// os-js crate (JS target, future)
handl "fs.read" = js_fs_read          // Node.js fs.readFile
```

The host selects the impl crate based on target. The `.hnl` interface,
axioms, proofs, capabilities, and doc comments are shared across all
targets — only the `handl` bindings change.

**Runtime support:**

- **Interpreted (tree-walk):** works today — `op` dispatches to
  `ModuleLookup`.
- **JIT (cranelift):** `op` lowers to `ArrSource::Foreign` in Scheme IR
  — same dispatch path, different eval engine.
- **WASM:** `ws-whatwg-wasm` already proves the pattern via
  `cfg(target_arch = "wasm32")`.
- **JS:** a future `plugin-lib-X-js` impl crate would register JS-backed
  handls under the same `op` names.
- **Sandbox:** the `cap` declarations + `requires`/`guarantee`
  enforcement (Phase 3) form the sandbox boundary — a sandboxed host
  refuses to register handls for caps it doesn't allow.

The `op`/`handl` split _is_ the target abstraction. The interface
(`.hnl`) is target-neutral; the impl (Rust handls) is per-target. No
additional mechanism is needed for multi-target support — it is
inherent in the design.

## What stays Rust forever

- **Native function bodies** — the actual I/O, crypto, syscall, TLS
  handshake implementations. These are irreducible; handl cannot
  implement `fs.read` or a TLS handshake.
- **The `plugin-lib-handl` crate** — the generic loader itself.
- **The engine crates** (`plugin-engine-lex-pattern`,
  `plugin-engine-parse-pratt`, `plugin-engine-eval-tree-walk`, …) — the
  language runtime.
- **The host crates** (`host-cli`, `host-lsp`, `host-tooling`) — the
  platform.

Everything else — logic, declarations, contracts, proofs, capabilities,
doc comments — lives in `.hnl` files.

## Consequences

### Positive

- **Massive Rust deletion.** ~60 `plugin-lib-*` crates collapse to ~1
  generic loader crate + N `.hnl` files + a small number of Rust
  `Function` bodies for native primitives.
- **Proofs where Rust can't offer them.** Every public arrow carries an
  axiom and (for handl-authored logic) a compositional proof. Native
  `op`s carry trusted axioms enforced at the boundary.
- **Single source of truth.** A library's logic, types, contracts,
  capabilities, and documentation all live in one `.hnl` file. No Rust /
  handl duplication.
- **ADR-0069 Phase 7 completed.** The arrow-native module system reaches
  its intended end state: stdlib as importable `.hnl` files.
- **Compositional inference end-to-end.** Contracts cache per module;
  inference scales across the import graph without re-tracing.

### Negative / trade-offs

- **`op` axioms are trusted, not proven.** The Rust body (the handl) is assumed
  to honor the declared `guarantee`/`requires`. A bug in the Rust body
  that violates the axiom is not caught by the proof system — only by
  runtime `requires` checking (Phase 3) or testing.
- **Runtime `requires` checking has overhead.** Checking predicates at
  every `Foreign` call adds cost. May need a mode that disables runtime
  checks in production (trusting static inference + the axiom).
- **Two-phase migration.** Pure-logic libs migrate first (Phase 1);
  native libs wait for Phase 2–4. During the interim, the stdlib is a mix
  of `.hnl`-loaded and Rust-registered modules.
- **Surface syntax growth.** `op` and `cap` declarations add
  two new top-level forms to the data language. The Pratt parser and
  lexer must support them.
- **Rust `Function` bodies are now isolated.** Each native `Function`
  impl is a leaf with no trait scaffolding around it. This is simpler but
  loses the `Module`-level grouping in Rust (the grouping moves to
  `.hnl`).

### Compatibility

- **Pure-logic lib migration (Phase 1)** preserves the public API of each
  crate (proven with `plugin-lib-stats`). Downstream crates that depend
  on `plugin-lib-stats::LibraryPlugin` etc. continue to work.
- **Native lib migration (Phase 4)** changes the crate structure: the
  `Module`/`Plugin` trait impls are deleted, replaced by `Function` impls
  - `.hnl` interface. Downstream crates that depend on the `Module`/
    `Plugin` types will need to switch to the `HandlLibraryPlugin` /
    `HandlModule` API. This is a breaking change for native lib crates,
    gated behind Phase 4.
- **`op` and `cap` syntax (Phases 2, 5)** are additive — new
  declaration forms that don't affect existing `.hnl` files.

## Rationale

The stdlib today is mostly Rust scaffolding around handl logic. The
scaffolding exists because the eval-side module registry and the
inference-side module loader were never connected. `HandlModule` (just
built) closes that gap for pure-logic libs. For native libs, the gap is
wider: there is no handl surface syntax for foreign declarations, and
axioms are not enforced at the eval boundary.

This ADR commits to closing both gaps. The payoff is not just Rust
deletion — it is moving the stdlib's correctness story (contracts,
proofs, capabilities) into handl, where it can be verified, and out of
Rust, where it cannot. The Rust that remains is pure implementation,
trusted to honor the contracts that handl declares and enforces.

The phase ordering ensures each step is independently shippable and
reversible. Phase 1 (pure-logic libs) is already in progress and proven.
Phases 2–3 (FFI + axioms) are the enabling work for native lib migration.
Phases 4–5 complete the migration. Phase 6 is cleanup that ADR-0069
already anticipated.

## Related ADRs

- [ADR-0069 — Arrow-Native Modules](./0069-arrow-native-modules.md) — the
  module system this migration targets (Phase 7).
- [ADR-0052 — Scheme IR Contracts](./0052-scheme-ir-contracts.md) — the
  `ArrSource::Foreign` / `Scheme` IR this builds on.
- [ADR-0068 — Predicates as Types](./0068-predicates-as-types.md) — the
  refinement types axioms use.
- [ADR-0051 — FFI Rust Binding Facade](./0051-ffi-rust-binding-facade.md)
  — prior FFI design; this ADR supersedes its Rust-side declaration model
  by moving declarations into handl.
- [ADR-0070 — Refinement Inference Scope](./0070-refinement-inference-scope.md)
  — the inference engine that consumes the contract cache.

## Appendix: Full stdlib inventory and migration map

Every `plugin-lib-*` crate, its category, approximate Rust line count,
and migration path. Categories:

- **Pure** — logic is (or can be) handl-authored; no Rust crate deps.
  Migrates via `HandlModule` (Phase 1).
- **Mixed** — some functions are pure handl, others wrap Rust crates.
  Migrates in two steps: pure functions via `HandlModule` (Phase 1),
  native functions via `op`/`handl` (Phase 4).
- **Native** — all functions wrap Rust crates or syscalls. Migrates via
  `op`/`handl` FFI (Phase 4).
- **Interface** — defines the API surface (module/function names),
  delegates to a `-std` or native variant. Migrates to a `.hnl` file
  with `op` declarations (Phase 4); the delegate crate provides the
  handls.
- **Special** — engine crates, the loader itself. Stays Rust.

### Already migrated

| Crate              | Lines | Status                                       |
| ------------------ | ----- | -------------------------------------------- |
| `plugin-lib-stats` | 390   | Migrated to `HandlModule` (proof-of-concept) |
| `plugin-lib-handl` | 637   | The generic loader itself — stays Rust       |

### Pure-logic (Phase 1)

| Crate                   | Lines | Notes                                                |
| ----------------------- | ----- | ---------------------------------------------------- |
| `plugin-lib-hello`      | 329   | Has `.hnl`, uses `include_str!` — simplest migration |
| `plugin-lib-dedent`     | 268   | String manipulation, no native deps                  |
| `plugin-lib-type`       | 195   | Type introspection helpers, no native deps           |
| `plugin-lib-compress`   | 100   | Interface to compress-gzip                           |
| `plugin-lib-kv`         | 305   | Interface to kv-hashmap                              |
| `plugin-lib-kv-hashmap` | 186   | In-memory key-value store, pure logic                |
| `plugin-lib-encoding`   | 97    | Interface to base64/hex                              |
| `plugin-lib-archive`    | 110   | Interface to archive-tar                             |

### Mixed — pure logic + native ops (Phase 1 for pure parts, Phase 4 for native parts)

| Crate                      | Lines | Native deps             | Notes                                                                                    |
| -------------------------- | ----- | ----------------------- | ---------------------------------------------------------------------------------------- |
| `plugin-lib-collections`   | 4446  | Rust `Vec`/`HashMap`    | Has `.hnl`; array/list/deque/stack/queue/map/set — some pure, some wrap Rust             |
| `plugin-lib-utils`         | 10165 | Rust `Math`, string ops | Has `.hnl`; fn/math/str/obj/type/cmp/bool/option/result/set/mood/parse/serialize — mixed |
| `plugin-lib-cmp`           | 893   | Rust comparison         | Has `.hnl`; comparison logic — mostly pure, some native                                  |
| `plugin-lib-os`            | 3011  | Interface only          | Has `.hnl`; delegates to `os-std` — interface crate                                      |
| `plugin-lib-http-client`   | 853   | Interface only          | Has `.hnl`; delegates to `http-client-std`                                               |
| `plugin-lib-http-server`   | 819   | `hyper`                 | Has `.hnl`; server logic + native HTTP                                                   |
| `plugin-lib-json`          | 180   | `serde_json`            | Has `.hnl`; parse/stringify logic + native serde                                         |
| `plugin-lib-toml`          | 177   | `toml` crate            | Has `.hnl`; parse/stringify + native                                                     |
| `plugin-lib-yaml`          | 179   | `serde_yaml`            | Has `.hnl`; parse/stringify + native                                                     |
| `plugin-lib-crypto`        | 261   | `sha2`/`aes`            | Has `.hnl`; hash/encrypt ops                                                             |
| `plugin-lib-datetime`      | 1228  | `chrono`                | Has `.hnl`; date/time logic + native chrono                                              |
| `plugin-lib-rand`          | 3182  | `rand` crate            | Has `.hnl`; random generation                                                            |
| `plugin-lib-unicode`       | 459   | `unicode` libs          | Has `.hnl`; unicode ops                                                                  |
| `plugin-lib-intl`          | 4957  | `icu`                   | Has `.hnl`; internationalization                                                         |
| `plugin-lib-otlp`          | 984   | `otlp`                  | Has `.hnl`; telemetry                                                                    |
| `plugin-lib-test`          | 1238  | native                  | Has `.hnl`; test framework                                                               |
| `plugin-lib-kv`            | 305   | interface               | Has `.hnl`; delegates to kv-hashmap                                                      |
| `plugin-lib-ws-rfc-memory` | 602   | in-memory               | Has `.hnl`; pure websocket frame logic                                                   |

### Native — all Rust (Phase 4)

| Crate                         | Lines | Native deps         | Notes               |
| ----------------------------- | ----- | ------------------- | ------------------- |
| `plugin-lib-base64`           | 126   | `base64`            | Encode/decode       |
| `plugin-lib-hex`              | 123   | `hex`               | Encode/decode       |
| `plugin-lib-ip`               | 1326  | `ipnet`             | IP/CIDR/socket      |
| `plugin-lib-parse`            | 84    | native              | Parser utilities    |
| `plugin-lib-prim`             | 383   | native              | Primitive ops       |
| `plugin-lib-serialize`        | 151   | `serde`             | Serialization       |
| `plugin-lib-tag`              | 1026  | native              | Tagging system      |
| `plugin-lib-tls`              | 762   | `rustls`            | TLS interface       |
| `plugin-lib-tls-native`       | 533   | `rustls`            | TLS implementation  |
| `plugin-lib-url-rfc`          | 216   | `url`               | RFC URL parsing     |
| `plugin-lib-url-whatwg`       | 217   | `url`               | WHATWG URL parsing  |
| `plugin-lib-archive-tar`      | 281   | `tar`               | Tar archive         |
| `plugin-lib-compress-gzip`    | 224   | `flate2`            | Gzip compression    |
| `plugin-lib-arrow-iter`       | 204   | native              | Arrow iteration     |
| `plugin-lib-concurrent`       | 6622  | `tokio`             | Concurrency         |
| `plugin-lib-os-std`           | 936   | `std::fs`/`process` | OS operations       |
| `plugin-lib-http-client-std`  | 138   | `reqwest`           | HTTP client impl    |
| `plugin-lib-http-server-std`  | 491   | `hyper`             | HTTP server impl    |
| `plugin-lib-otlp-std`         | 468   | `otlp`              | OTLP implementation |
| `plugin-lib-ws-rfc`           | 2407  | native              | WebSocket RFC       |
| `plugin-lib-ws-rfc-native`    | 1505  | native              | WebSocket native    |
| `plugin-lib-ws-whatwg-native` | 423   | native              | WebSocket WHATWG    |
| `plugin-lib-ws-whatwg-wasm`   | 486   | native              | WebSocket WASM      |

### Special — stays Rust (no migration)

| Crate                          | Lines | Notes                          |
| ------------------------------ | ----- | ------------------------------ |
| `plugin-lib-lang-compile`      | 554   | Language engine — compile      |
| `plugin-lib-lang-eval`         | 514   | Language engine — eval         |
| `plugin-lib-lang-lex`          | 513   | Language engine — lex          |
| `plugin-lib-lang-parse`        | 488   | Language engine — parse        |
| `plugin-lib-lang-run-compiled` | 493   | Language engine — run compiled |
| `plugin-lib-lang-typecheck`    | 534   | Language engine — typecheck    |
| `plugin-lib-handl`             | 637   | The generic loader itself      |

### Migration summary

| Category         | Crates | Total Rust lines | Migration phase | Rust after migration     |
| ---------------- | ------ | ---------------- | --------------- | ------------------------ |
| Already migrated | 2      | 1027             | Done            | ~637 (loader only)       |
| Pure-logic       | 8      | ~1590            | Phase 1         | 0 (all `.hnl`)           |
| Mixed            | 18     | ~23900           | Phase 1 + 4     | `op` handls only         |
| Native           | 23     | ~17600           | Phase 4         | `op` handls only         |
| Special          | 7      | ~3733            | Never           | stays Rust               |
| **Total**        | **58** | **~47850**       |                 | **~3733 + handl bodies** |

After migration, ~44,000 lines of Rust scaffolding (Module/Plugin/Function
trait impls, bridge code, per-lib runtime construction) is deleted. What
remains is ~3,733 lines of engine/loader code + the native `Function`
bodies (the handls) + N `.hnl` files carrying all logic, contracts,
proofs, capabilities, and documentation.
