# ADR-0128: Target repos — ports stay in `platform`, modules live in ecosystem repositories

## Context

The `handl` repository is a single Cargo workspace of **90 members** —
engine substrate, ports, adapters, hosts, and vendored language packages —
with 209 third-party crates in the resolve graph and 129 integration test
binaries. A full nextest run is ~71 minutes on this 11 GB host; `target/`
is 2.9 GB.

The motivation for splitting is fourfold and measured, not aesthetic:

- **Heavy optional deps in everyone's build.** `handl-kit-canonical`
  hard-depends on `handl-plugin-engine-exec-cranelift` (its *only* normal
  dependency — `handl-runtime`, `handl-kit-data-canonical` and
  `handl-defining-handl` reference it only in `[dev-dependencies]`). Every
  consumer of the canonical kit — the LSP host, `plugin-compile-js`, any
  parse-only embedder — therefore compiles **14 cranelift crates**.
- **Implementation detail in platform wiring.** `plugin-ext-sdkgen` is
  486 lines of which **205 are Python-specific** (`PythonSpec`,
  `load_python_spec`, `generate_python`, `build_python_hnl`,
  `build_code_assignment`) — a target's internals living inside the
  dispatcher.
- **Ownership / contributors.** No boundary exists between "platform"
  and "integration" that an outside contributor could be scoped to.
- **CI time.** One workspace = one graph; a change anywhere rebuilds
  everywhere.

The seams are already clean *at the crate level*. The graph is dense —
only 9 leaves, `handl-platform` has 57 inbound deps, `handl-language` 49 —
but the *port* layer is real: `plugin-sdkgen` declares
`SdkgenBackend`/`Emitter`/`DocSource`, `plugin-compile` declares
`Compiler`/`CompilerRegistry`, `plugin-lib-kv` declares `KeyValueStore`,
`plugin-ext-kit` declares `LibraryRegistry`.

What blocks independence is **three static registration tables**:

1. `plugin-ext-sdkgen/src/lib.rs` — `if kind != "c" && kind != "webidl" &&
   kind != "python"` plus a ~90-line `match` arm per target.
2. `plugin-ext-compile/src/dispatcher.rs` — `registry.register(Box::new(
   JsCompiler)); registry.register(Box::new(LlvmCompiler::new()));`
3. `plugin-ext-kit/src/library_registry.rs` —
   `reg.register("otlp-std", || Box::new(...))`, `kv-hashmap`, …

…and `host-cli/Cargo.toml` statically depends on `plugin-compile-js` /
`plugin-compile-llvm`.

The `.hnl` side has its own constraint: **68 of 95 packages are
referenced from `crates/`** and 31 of those are `include_str!`-pinned to
`plugin-lib-stdlib`'s `ModuleSpec` table (`hnl: &'static str`), which
makes them compile-time welded to the stdlib crate.

## Decision

**Repo = target/ecosystem; module = one implementation of one port; a
module registers one or more named targets.** Target name ≠ repo name.

**Module directories carry the specific name, never the generalized
port name.** The convention is `modules/<port>-<specific>`: the port is
a *namespace prefix*, not a rename. `web/modules/sdkgen-webidl`
registers the `webidl` target — and when `web/` later grows a second
sdkgen target (dts, cssom, …) it lands beside it as
`sdkgen-<that>` without colliding. Likewise
`native/modules/exec-cranelift` / `native/modules/compile-llvm` keep
their dep's name, and `dot/`/`term/`/`sexp/` are three separate repos —
repo-per-target consistency for compiler targets; each holds
`modules/compile-<target>` so the dir name stands alone.

Platform keeps: ports, core (`handl-platform`, `handl-language`,
`handl-value`, `handl-runtime`), the generic kit machinery
(`handl-kit-data` — kit/fragment *types*, consumed by `plugin-ext-kit`
which stays) and `handl-kit-arrow-lower` (the `Ast → Term` lowerer —
`handl-runtime` links it, and `lang/` must not create a
platform↔lang cycle), engines (`plugin-engine*`), wiring
(`plugin-ext-*` — minus the static tables and
target internals), loaders/appliers, `host-tooling` (the shared host
contract both host repos depend on), and **the three `plugin-lib-*`
crates that are machinery, not content**:
`plugin-lib-handl` (the generic `.hnl` module loader —
`HandlModule`/`OpRegistry`; 19 inbound deps, including every moving
lib), `plugin-lib-tag` (tagged-values host primitive used by the
typecheckers), `plugin-lib-scheduler` (test-engine machinery). Every
other `plugin-lib-*` moves to `stdlib/`.

### Extraction list

| what | → repo / module | port |
|---|---|---|
| `plugin-sdkgen-backend-c` | `c/modules/sdkgen-c` | `SdkgenBackend` |
| `plugin-sdkgen-backend-webidl` | `web/modules/sdkgen-webidl` | `SdkgenBackend` |
| `plugin-sdkgen-emitter-python` + the 205 Python lines of `plugin-ext-sdkgen` | `python/modules/sdkgen-python` | `Emitter` |
| `plugin-compile-js` | `web/modules/compile-js` | `Compiler` |
| `plugin-compile-llvm` | `native/modules/compile-llvm` | `Compiler` |
| `plugin-engine-exec-cranelift` | `native/modules/exec-cranelift` | exec engine |
| `plugin-compile-dot` | `dot/modules/compile-dot` | `Compiler` |
| `plugin-compile-term` | `term/modules/compile-term` | `Compiler` |
| `plugin-compile-sexp` | `sexp/modules/compile-sexp` | `Compiler` |
| `plugin-compile-handl` | `handl/modules/compile-handl` | `Compiler` |
| `plugin-lib-otlp`, `plugin-lib-otlp-std` | `otlp/modules/{otlp,otlp-std}` | `LibraryPlugin` |
| `plugin-lib-kv-hashmap` | `stdlib/modules/kv-hashmap` | `KeyValueStore` |
| `plugin-lib-stdlib` + its 31 `include_str!`ed packages | `stdlib/crates/plugin-lib-stdlib` + `stdlib/packages/` | module chain |
| `plugin-lib-kv` | `stdlib/modules/kv` | kv port — its only consumer is `kv-hashmap`, so the port rides to stdlib |
| `plugin-lib-{crypto,encoding,parse,serialize,char-native}` | `stdlib/modules/{crypto,encoding,parse,serialize,char-native}` | library impls |
| `plugin-lib-lang-{lex,parse,eval,typecheck,compile,run-compiled}` (6) | `stdlib/modules/lib-lang-*` | language toolchain modules |
| `.hnl` packages pinned to the moving libs — `crypto`, `encoding`, `parse`, `serialize`, `kv`, `kv_hashmap`, `lang_*`, `char`/`char-native` | `stdlib/packages/` | (moves with crate) |
| `.hnl` packages `otlp`, `otlp_std` | `otlp/` | (moves with crate) |
| `.hnl` web block (`js`, `fetch-js`, `ws_frame{,-handl}`, `ws-c`, `ws-whatwg-c`, `ws_whatwg_wasm`, `http_rfc`, `serve{,-epoll}`, `concurrent-epoll`) | `web/` | (not pinned) |
| `generated/*-webidl-browser` (13 dirs) | `web/` | (not pinned) |
| `host-cli` + `plugin-ext-cli` | `cli/` | the `hnl` distribution — its dep list *is* its target set |
| `host-lsp` + `plugin-lsp` + `plugin-lsp-std` + `plugin-ext-lsp` | `lsp/` | the LSP server distribution |
| `plugin-ffi` | `ffi/modules/ffi` | FFI marshalling — sole normal consumer is `kit-canonical`; pulls `cranelift` + `libloading`, the last heavy deps in platform |
| `handl-kit-data-canonical` + canonical `data/*.yaml` + `handl-packages/` remainder | `lang/` | Handl-the-language as data (ADR-0044 made literal) |
| `kit-canonical` | `kit-canonical/crates/kit-canonical` | the canonical *kit assembly* — wires language data + stdlib + engines into a `Kit`; consumed by hosts |
| `handl-defining-handl`, `plugin-engine-typecheck-nbe` | `meta/crates/` | self-hosting/NbE experiments — zero-consumer leaves |
| `docs/` (VitePress site) + the sibling `handl-lang/docs/` (top-level ADRs, outside the platform repo) | `docs/` | documentation hub; each repo's API reference is generated from its workspace tree and published into `docs/` |

`plugin-compile-handl` moves to **`handl/`** — the repo for Handl as a
target — under strict target-per-repo consistency. **Naming:** the
platform repo (today `handl-lang/handl`) is renamed **platform/**, since
"platform" is exactly what it now holds; `handl/` then means the
*language target*, not the workspace. `host-tooling` stays — it is the
shared host contract
consumed by *both* `cli/` and `lsp/`; sole-consumer satellites ride to
their host's repo (`plugin-ext-cli` → `cli/`; `plugin-lsp`,
`plugin-lsp-std`, `plugin-ext-lsp` → `lsp/`).

**Hosts sit at the top of the fan.** `host-cli`'s 74-crate closure and
`host-lsp`'s 48 become explicit git-dep lists in their repos: the `hnl`
binary's target set is exactly what `cli/` declares. This also makes the
registries load-bearing — the backends are no longer in-tree to fall
back on.

### Registration: link-time first

Phase one keeps the ports' existing `register()` surfaces but drives
them from a **feature-gated target registry** in `plugin-sdkgen` /
`plugin-compile` / `plugin-ext-kit`: the host opts targets in via Cargo
features on its own manifest, each feature enabling a repo module. The
`plugin-loader-native` dylib path is the documented future for true
zero-repackage plugin delivery.

A consequence in the sdkgen namespace: a *target* composes a backend and
an emitter, which may come from different repos — `hnl sdkgen python` is
backend `c` + emitter `python`. The registry key is the target record
`(backend_id, emitter_id)`, so `python/` depends on `c/`'s module at
resolution time, not at the crate level.

### `.hnl` ownership rule

One `include_str!` per package, in the crate that owns it; every other
consumer takes a `pub const` re-export. `hello`, `transform` and `stats`
are today pinned by two or more crates — they get a single owner when
`stdlib/` moves. `handl-packages/` itself **splits**: packages pinned by
staying crates stay in platform; the 31 stdlib-pinned ones move with
`plugin-lib-stdlib`; unpinned ecosystem packages move with their repo.

### Resolution

Git deps + `[patch]` for local development — **local only, no
publishing** for now:

```toml
handl-plugin-sdkgen = { git = "https://github.com/handl-lang/platform", tag = "v0.1.0" }

[patch."https://github.com/handl-lang/platform"]
handl-plugin-sdkgen = { path = "../platform/crates/plugin-sdkgen" }
```

`[patch]` is local-dev only; CI resolves the tag. A `dev-link` script
generates the patch block across sibling checkouts. Manifests gain
`license`/`description`/`repository` now so future publishing is a no-op.

## Consequences

**Good.** Each repo builds only its own graph; `cranelift` and
`clang-sys`/`inkwell` leave the default build; `plugin-ext-sdkgen` loses
205 foreign lines; ownership boundaries exist; the C-FFI `.hnl` block
unblocks because the pins move *with* `plugin-lib-stdlib` — no
module-chain seam needed for those.

**Costs.** A change to a port (`Compiler`, `SdkgenBackend`) becomes a
multi-PR atomic change plus a git rev bump — mitigated by
`tag`-pinning, not eliminated. `kit-canonical` gains a git dependency on
`stdlib/` — a deliberate inversion: the platform now depends on the
stdlib package, not vice versa. Shared `.hnl` sources pinned by multiple
crates need the `pub const` re-export rule before any move.

**Private-repo auth.** Every repo is private except **`docs/`** (the published
hub). `platform/` was public during the transition and `cli/` was created
public; both are private now. Git deps bypass registries entirely —
`cargo` just clones — so consumers need `CARGO_NET_GIT_FETCH_WITH_CLI=1` (or
`net.git-fetch-with-cli` in `.cargo/config.toml`) for ssh auth, and each
repo's CI needs a read credential for its sibling deps. Cheap today;
flip to public when the "external contributor" motivation becomes real.

**The `lang/` cycle.** `plugin-engine-test`, `plugin-ext-lint`,
`plugin-ext-typecheck` and `host-tooling` have *normal* deps on
`kit-canonical` — and `kit-canonical` depends on platform engines. A
naive move creates a cross-repo cycle. The seam that breaks it is the
same one ADR-0099 established: those consumers take `&Language`/a kit
handle as a parameter and the concrete canonical kit is assembled in
the `kit-canonical/` repo (which `cli/`/`lsp/` git-depend on alongside
`lang/`). This injection is Phase-0 work, not a `lang/`-phase surprise.

**The `0.8` seam, concretely** (measured against the real call sites,
2026-10-15):

- **`Plugin: Send + Sync` is the binding constraint.** A `Kit` holds
  `RefCell`s and is `!Sync`, so no plugin struct may hold one — today
  or ever. Injection therefore cannot be "store the kit"; it is:
  - a **`&Language`/`Arc<Language>`** where the consumer only reads the
    language definition (`Language: Send + Sync` — proven by the
    existing `static LANG: OnceLock<Language>` in `plugin-ext-lint`), or
  - an **`Arc<dyn Fn(...) -> ... + Send + Sync>` capability closure**
    where the consumer needs kit *behavior* (e.g.
    `kit.typecheck_handl_with_prelude`,
    `kit.handl_lint_adapter().resolver()`, `kit.lex_with_spans`).
  - The host keeps today's per-thread memoization: it owns a
    `thread_local! RefCell<Option<Kit>>` and injects a *capture-less*
    closure (`|src, p| KIT.with(|k| …)`) — capture-free, so the
    closure is `Send + Sync` while the kit stays thread-local. The
    plugin becomes a pure adapter with no kit types in its signature.
- **`plugin-engine-test` needs a factory, not a value.** Scheduler
  workers each build their own `Runtime` on-thread (`thread_local!`,
  because `Rc<dyn ModuleLookup>` cannot cross threads), so the runner
  must receive something it can invoke *inside* the job closure:
  `Arc<dyn Fn() -> Result<Runtime, String> + Send + Sync>`. `HandlModule`
  and `with_loader_disabled` already live in `plugin-lib-handl` (the kit
  merely re-exports them) — those imports just change paths.
- **`host-tooling` is the worst current shape:** `BasicSemanticTokens::
  run` assembles `Kit::new()` **per LSP request**. The plugin must hold
  the injected lex capability from construction; `SpannedToken`/
  `TokenClasses` are kit-defined types and either move to a lower crate
  or the injection is closure-shaped over them.
- **`dev-dependencies` on `kit-canonical` are legal and stay.** Cargo
  permits cyclic dev-deps, and these four crates' tests genuinely want
  the canonical kit. Only *normal* deps create the platform↔lang cycle;
  the work is severing exactly those four.
- **Hosts keep their normal dep.** `host-cli`/`host-lsp` move to
  `cli/`/`lsp/` anyway, where a git dep on `lang/` is the intended
  direction — they are the assembly point that injects.
- **Precedent already exists:** `plugin-ext-fmt`'s
  `*_with_language(lang, src)` variants and
  `lint_source_with_language`/`lint_source_with_prelude_and_language`
  are the same seam in miniature. The 0.8 change extends that
  convention to the whole surface rather than inventing a new one.

**Deferred.** `plugin-loader-native` dylib registration (true
zero-repackage); the `FragmentSource::DataFile` module-chain seam (still
`NotImplemented`) for `.hnl`-only packages that live outside any crate;
`c/`'s 6 generated `*-c-linux-x86_64/` dirs and the remaining `-c`
package block pending item-7 scope; `graphql/`/`pg/`/`sqlite/` as
greenfield repos once a first module exists.

## Status

Proposed. Phase 11 executed — `lang/` + `kit-canonical/` extracted to
sibling repos (`git subtree split` history, git deps + workspace-root
`[patch]` for sibling-checkouts, `url.insteadOf` +
`CARGO_NET_GIT_FETCH_WITH_CLI` for pre-push local resolution).
