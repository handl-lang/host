# Task 0128 — Target-repo split (ADR-0128)

Split `platform` into the reference platform plus target/ecosystem repos.
Repo = target; module = one implementation of one port; modules register
named targets. Resolution: git deps + `[patch]`, local-only. See
`project/adrs/0128-target-repos.md` for the full extraction list.

## Phase 0 — platform prerequisites (no new repos)

- [x] **0.1 `SdkgenTarget` descriptor + registry** in `plugin-sdkgen`:
      `name`, flags, config loader, backend, emitter, doc source.
      `plugin-ext-sdkgen` collapses to a lookup over it. Delete the
      `kind != "c" && …` gate and the per-target `match` arms.
      **Done:** `plugin-sdkgen/src/targets.rs` adds `SdkgenRequest` (the
      parsed CLI context incl. the injected `&Language`),
      `SdkgenBackendDef`/`SdkgenEmitterDef` factories, `SdkgenSpecInput`
      (non-backend inputs), the `SdkgenTarget` record (`name`,
      `source_filters`, `backend`/`emitter` *ids*, `docs`, `spec_input`),
      and `SdkgenRegistry::run` — the shared parse → provenance → docs →
      diagnostics → emit pipeline, with `SdkgenTargetError::{Usage,
      Failed}` mapping to exit 2/1. Each module exports `register(reg)` —
      `c/modules/sdkgen-c`, `web/modules/sdkgen-webidl`,
      `python/modules/sdkgen-python`, `plugin-sdkgen-emitter-file`
      (`hnl-file`). `plugin-ext-sdkgen` is now flag-parse + `--from-url`
      fetch + `registry.run`; the `kind !=` gate is a registry lookup and
      the usage string derives from `registry.target_names()`. Each target
      dep is a feature (`c`/`webidl`/`python`, all default; `python`
      implies `c` since its record names backend `c` — the cross-repo
      composition is name-level, per the ADR).
- [x] **0.2 Move the Python spec path** (205 lines: `PythonSpec`,
      `load_python_spec`, `generate_python`, `build_python_hnl`,
      `build_code_assignment`) out of `plugin-ext-sdkgen` — it goes to
      `python/modules/sdkgen-python` in Phase 1.
      **Done:** `python/modules/sdkgen-python/src/spec.rs` holds the moved
      machinery; `generate_python`/`is_python_spec` are the target's
      `SdkgenSpecInput` hooks. (Phase-1's "already its own crate" note was
      about the *emitter* — the spec path was still in ext-sdkgen and is
      now actually moved.)
- [x] **0.3 `CompileDispatcher` → target registry** in `plugin-compile`:
      no static `register(Box::new(JsCompiler))`; targets opt in via a
      feature-gated registry on the host's manifest.
      **Done:** `plugin-ext-compile::linked_targets()` registers one
      `#[cfg(feature)]` per repo module (`js`/`llvm`/`handl`/`term`/`dot`/
      `sexp`, all default); `CompileDispatcher` gains
      `with_registry`/`registry_mut`/`into_registry`/`targets` for
      host-assembled sets. `host-cli` (cli repo) exposes the six as
      pass-through manifest features — the `hnl` binary's target set is
      literally its feature list — and its compiler deps now git-dep
      their own repos.
- [x] **0.4 `library_registry.rs` → data-driven**: kit YAML `modules:`
      resolves names through a registry the kit supplies, not a static
      `reg.register` table.
      **Done:** `BasicKitLoaderPlugin::new(module_names, libraries)` takes
      a populated `Arc<LibraryRegistry>` — `load()` resolves `modules:`
      names through it instead of rebuilding the static table per call.
      The canonical table moved to the kit repo:
      `handl-kit-canonical::populate`/`canonical_library_registry()`
      (`kit-canonical/crates/kit-canonical/src/library_registry.rs`).
      `plugin-ext-kit` dropped its `plugin-lib-{stdlib,crypto,kv-hashmap,
      otlp-std}` deps entirely. `DefaultApplierFactory`/
      `DefaultApplierPlugin`/`BootstrapPlugin` thread the registry; host-cli
      injects `canonical_library_registry()`.
- [x] **0.5 Drop `exec-cranelift` from `kit-canonical`** — it is the
      crate's only normal dep on it; select the exec engine via the ext
      contract. This removes 14 cranelift crates from every consumer.
      Verify `plugin-lsp-std`'s closure shrinks accordingly.
      **Done:** the dep is now `optional`, behind the `cranelift` feature
      (`default = ["cranelift"]` keeps today's behavior). Engine
      selection already ran through the `EngineRegistry` name contract —
      the two duplicated registration tables are deduplicated into
      `canonical_engine_registry()`, and `Kit::new_with_registry` /
      `runtime_for_lang_with_registry` are the injection seam for hosts
      that opt out and wire their own exec engine. The LSP closure
      shrink lands with 0.9 (`plugin-ffi` also pulls cranelift crates).
- [x] **0.6 Manifests**: add `license`, `description`, `repository` to
      every crate; git-tag `v0.1.0`.
      **Done (manifests; tag deferred to the user):** `[workspace.package]`
      declares `license = "MIT OR Apache-2.0"` +
      `repository = "https://github.com/handl-lang/platform"`; all 90
      packages (89 `crates/**/Cargo.toml` + the root `handl` package)
      inherit via `license.workspace`/`repository.workspace` and carry a
      `description` (extracted from each crate's `//!` doc line;
      hand-written for the 9 undocumented crates). Ready for `v0.1.0` —
      **note:** no `LICENSE` files exist in the repo yet; the declared
      SPDX expression assumes the intended dual license and the files
      should be added before crates.io publishing.
- [x] **0.7 `pub const` re-export rule**: each `include_str!`ed package
      gets exactly one owner; other consumers take `pub const`. Apply to
      `hello`, `transform`, `stats` first (each pinned by ≥2 crates).
      **Done:** `hello`/`transform`/`stats` resolved by the Phase-3/11
      moves — `plugin-lib-stdlib` owns them and exports
      `GREET_HNL`/`TRANSFORM_{MAP,FILTER,REDUCE}_HNL`; no platform crate
      pins them anymore. In-platform remainder applied: `str-handl`'s
      second pin (`plugin-lib-handl/examples/loader_bench.rs`) now takes
      `pub const STR_HANDL_HNL`. Also fixed a split-introduced breakage:
      `plugin-engine-typecheck-handl`'s parity test `include_str!`ed the
      moved `examples/handl/lib-utils/hello.hnl` — vendored as
      `tests/fixtures/hello.hnl` (same pattern as compile-handl's
      fixtures).
- [x] **0.8 Kit-injection seam**: `plugin-engine-test`,
      `plugin-ext-lint`, `plugin-ext-typecheck`, `host-tooling` take a
      `&Language`/kit handle instead of linking `kit-canonical`
      directly — their normal deps on it would otherwise create a
      platform↔lang cycle in Phase 9. **Mechanism** (see ADR-0128 §"The
      0.8 seam, concretely"): `Plugin: Send + Sync` forbids holding a
      `Kit` — inject `Arc<dyn Fn + Send + Sync>` capability closures or
      `&Language`/`Arc<Language>`; the host owns `thread_local` kit
      caches and injects capture-less closures; `plugin-engine-test`
      needs a runtime *factory* (workers build thread-local runtimes);
      `host-tooling` must stop assembling `Kit::new()` per LSP request;
      dev-deps stay, only normal deps sever.
- [x] **0.9 `plugin-ffi` de-coupling**: `kit-canonical`'s dep on it is
      the only normal edge — keep it, but make the ffi-capable kit a
      declared feature so platform's default build drops
      cranelift/libloading even before `ffi/` exists.
      **Done:** `handl-plugin-ffi` is `optional`, behind the `ffi`
      feature (`default = ["cranelift", "ffi"]` keeps today's behavior).
      With `ffi` off the `ffi` module stays resolvable via stub ops —
      capability ops raise a clear "not compiled in" error while
      `ffi.load`/`ffi.sym` keep their non-raising `{__tag__: "err"}`
      probe contract so `unicode_c`/`intl_c`/`entropy` degrade to their
      fallback paths. `host-lsp` now builds the kit with
      `default-features = false` (it only lexes/parses/typechecks/lints),
      so cranelift + libloading are out of its closure — verified via
      `cargo tree -p handl-host-lsp`.
- [x] **0.10 Applier audit**: `plugin-applier-bootstrap` has zero
      consumers — `BootstrapPlugin` is never instantiated. It stays in
      platform (it's the ADR-0036 host-composition seam, and `cli/`
      will need it to assemble kits), but decide: **wire it** (make
      `host-cli` compose via `BootstrapPlugin` as designed) or
      **document superseded** (host-cli's inline path won). Do not
      delete blindly.
      **Done — wired, as designed.** The premise was stale: `host-cli`
      (now in `cli/`) already composes via it —
      `cli/crates/host-cli/src/main.rs` `run_via_applier` calls
      `host.register_plugin(BootstrapPlugin::new(config_path, kit_name,
      applier_id, module_names, libraries))`. The §0.4 work extended its
      signature; nothing deleted. Note: `plugin-applier{,-default,
      -bootstrap}` are *not* workspace members — they compile through the
      cli consumer; that is pre-existing, not a split artifact.

Phase-0 verification: `cargo check --workspace` green; `hnl sdkgen
c|webidl|python` and `hnl compile js|llvm|dot|term|sexp` behavior
unchanged (all still statically linked, just through registries).

## Phase 1 — pilot: `python/`

- [x] New repo `python`: workspace + `modules/sdkgen-python` crate
      (`Emitter` impl + the moved 205 lines) depending on
      `handl-plugin-sdkgen` via git dep; `[patch]` to sibling path for
      local dev.
      **Done:** `handl-lang/python` holds `modules/sdkgen-python`
      (subtree-split history; package name unchanged:
      `handl-plugin-sdkgen-emitter-python`). The Python spec code was
      already its own crate before the split, so no `plugin-ext-sdkgen`
      surgery was needed — the ext crate consumes the emitter via a
      pinned git dep, resolved locally through a new
      `[patch."https://github.com/handl-lang/python"]` block, and
      `handl-plugin-sdkgen` joined the platform `[patch]` table so the
      moved crate's port dep unifies with the in-tree package.
- [x] `hnl sdkgen python` resolves `python` → target record
      `(backend: c, emitter: python)`; cross-repo dependency on `c/`'s
      module is expressed at the *target* level.
      **Done:** `python/modules/sdkgen-python::register` contributes the
      record `{name: "python", backend: "c", emitter: "python",
      source_filters: true, docs: None, spec_input: yaml/json →
      spec::generate_python}`; `SdkgenRegistry::run` resolves backend
      `"c"` by name — `python/` has no crate dep on `c/` and
      `handl-plugin-sdkgen-backend-c` is not in `python/Cargo.toml`.
- [x] `dev-link.sh` (or superrepo) generates the `[patch]` block across
      sibling checkouts; CI uses the git dep only.
      **Done:** `platform/scripts/dev-link.sh [repo]` rewrites the
      `# dev-link: begin|end` region of the repo's root `Cargo.toml`:
      every `git = "…/handl-lang/R"` dep (plus one transitive hop —
      platform's own `[patch."…/platform"]` table exists because patched
      sibling manifests git-dep platform crates) gets every crate under
      `R/{crates,modules}/` patched to its sibling path. Platform's tables
      are now inside the markers and regenerated (14 tables, same
      entries, sorted).
- [x] NB: `python` works in Phase 1 because `backend: c` still resolves
      in-platform — Phase 2 re-points it to `c/`'s module. That
      re-resolution is the cross-repo composition proof, not an
      accident.
- [x] Prove: `python/` CI builds only its own graph; `platform/` tests
      green without the emitter.
      **Done:** `cargo check`/`test --workspace` green in `python/` (2
      ctypes-import tests); `cargo check --workspace` green in platform.

This phase validates the whole template on a 1-crate port surface with
0 external deps and simultaneously shrinks platform by 205 lines.

## Phase 2 — `c/`

- [x] `c/modules/sdkgen-c` ← `plugin-sdkgen-backend-c` (`clang-sys` leaves
      the platform lock/build).
      **Done:** subtree-split import; `plugin-ext-sdkgen` takes it via a
      pinned git dep + `[patch."https://github.com/handl-lang/c"]` →
      `../c/modules/sdkgen-c`. 88 tests green in the new repo.
- [x] Decide the C-ABI package block: `-c` packages and 6
      `generated/*-c-linux-x86_64/` dirs follow or stay (item 7).
      **Decided — stay (for now), and the reason is now concrete.** The
      `-c` packages (`tls-libssl`, `fetch-c`, `math-c`, `compress-*-c`, …)
      and `generated/{libc,curl,libssl,libz}-c-linux-x86_64` are pinned by
      *filesystem-relative* `.hnl` imports from the deferred web block
      (`concurrent-epoll`/`serve-epoll`/`ws-c`/`fetch-js` import
      `../generated/libc-c-linux-x86_64/*`, `../tls-libssl`, `../ws-c`,
      `../concurrent`, …). Moving them to `c/` while the web block stays
      would leave cross-repo `../../platform/…` imports — and moving them
      *with* the web block is exactly the Phase-5 decision. The real
      unblock for both is the deferred `FragmentSource::DataFile` seam
      (named resolution for crate-unowned packages). Revisit with
      Phase-5's `.hnl` web block, as one move.

## Phase 3 — `stdlib/`

- [x] `plugin-lib-stdlib` + its `include_str!`ed packages move to
      `stdlib/` (`crates/plugin-lib-stdlib` + `handl-packages/` — the dir
      name kept for consistency with `lang/` and `kit-canonical/`).
      **Done:** `handl-lang/stdlib` holds `crates/plugin-lib-stdlib`
      (subtree-split history) and 41 pinned package dirs + `char/`,
      `char-native/` (owned by `modules/char-native`, exported as
      `CHAR_HNL`/`CHAR_NATIVE_HNL` per the 0.7 rule) +
      `generated/icu-{c,uc}-linux-x86_64`. Platform keeps a fixture copy
      of `collections/array/` — `plugin-engine-typecheck-handl` (src)
      and `handl-runtime` tests `load_hnl_file` it by path.
- [x] The other plugin-lib content crates move as modules:
      `stdlib/modules/{kv,kv-hashmap,crypto,encoding,parse,serialize,
      char-native,lib-lang-{lex,parse,eval,typecheck,compile,
      run-compiled}}` — 14 modules total. **Exceptions — machinery stays
      in platform:** `plugin-lib-handl` (`.hnl` loader), `plugin-lib-tag`
      (host primitive), `plugin-lib-scheduler` (test engine).
      **Done:** all 14 moved with subtree-split history.
- [x] `kit-canonical`, `plugin-ext-kit`, `plugin-engine-test` and
      `plugin-engine-typecheck-handl` git-depend on `stdlib/` — the
      deliberate inversion. Wire via the same git-dep + `[patch]`
      **Done:** `plugin-ext-kit` (stdlib/crypto/kv-hashmap),
      `plugin-engine-test` (char-native), `plugin-engine-typecheck-handl`
      (stdlib; char-native dev), plus dev-deps in `handl-runtime`,
      `plugin-lib-handl`, `handl-defining-handl` (lib-lang-*),
      `plugin-ext-refinement` (char-native for `str_contract`). New
      `[patch."https://github.com/handl-lang/stdlib"]` block in the root
      `Cargo.toml` maps all 14 to `../stdlib/...`; kit-canonical repoints
      its 9 deps the same way. Fixed forward: stdlib/crypto
      `HandlModule`-evaluating tests that never installed the canonical
      language (pre-existing "no language available" failures on main —
      the 0.8 seam; `ensure_lang()` applied per `named_import.rs`'s
      pattern), and the same gap in `plugin-engine-typecheck-handl`'s
      `axis_flow_boundaries`/`dependent_structure`/`integration`/unit
      tests in platform.
      mechanism.

## Phase 4 — `otlp/`

- [x] `otlp/modules/{lib-otlp,lib-otlp-std}` ← `plugin-lib-otlp` +
      `-otlp-std` + `handl-packages/{otlp,otlp_std}` +
      `kits/server.yaml` fragment + `examples/otlp`.
      **Done:** subtree-split imports into `handl-lang/otlp`
      (`examples/otlp` rode inside `modules/lib-otlp/examples/`);
      `plugin-ext-kit` consumes `handl-plugin-lib-otlp-std` via a pinned
      git dep + `[patch."https://github.com/handl-lang/otlp"]`.
      **Deviation:** `data/kits/server.yaml` stays in `lang` — it is an
      embedded `KITS` row consumed by name-resolution tests; only the
      `FragmentSource::DataFile` seam (deferred) lets a module ship a
      kit manifest. `otlp-std` still resolves through the registry.

## Phase 5 — `web/`

- [x] `web/modules/sdkgen-webidl` ← `plugin-sdkgen-backend-webidl`
      (target `webidl`; CLI keeps the name).
      **Done:** subtree-split import; `plugin-ext-sdkgen` takes it via a
      pinned git dep + `[patch."https://github.com/handl-lang/web"]`.
- [x] `web/modules/compile-js` ← `plugin-compile-js` (target `js`).
      **Done:** subtree-split import; `plugin-ext-compile` takes it via
      a pinned git dep on the same rev. 45 tests green in `web/`.
- [x] `.hnl` web block + 13 `generated/*-webidl-browser/` dirs.
      **Done — the seam is `.handl.toml` `[dependencies]` name
      resolution** (Phase-9.5 package management), not a new
      module-chain registry: `from "ws-c"` / `from "libc/socket"`
      resolve the first segment through the manifest's name → dir map.
      Platform side: `handl-platform` gained `discover_manifest_dir` /
      `resolve_dependencies` / `discover_dependencies`; the runner's
      `import_preamble` and `host-cli`'s strip/seed helpers now consult
      the dep table, and the tooling kit is project-aware
      (`Kit::discover`). Moved: `web/` holds the web block
      (`js`, `fetch-js`, `ws_frame{,-handl}`, `ws-whatwg-c`,
      `ws_whatwg_wasm`, `http_rfc`, `serve{,-epoll}`, `concurrent-epoll`)
      + the `*-webidl-browser` adapters, their `generated/<spec>` ports,
      `.idl` inputs and webidl `*.sdkgen.yaml`; `c/` holds
      `concurrent`, `ws-c`, `tls-libssl`, `fetch-c`, `compress{,-gzip-c,
      -zlib-c}` and `generated/{libc,curl,libz,libssl}` ports+adapters,
      `curl-spec/` and the four c `*.sdkgen.yaml`. Cross-repo specs
      rewritten to dep names (`from "libc/socket"`, `from "ws-c"`,
      `from "concurrent-epoll"`); in-repo `../` imports unchanged.
      `math-c` stayed (its only edge is `../math`, still in platform);
      `generated/icu{,-uc}*` configs stayed (they generate stdlib-owned
      adapters — follow-up). dogfood-gate's ARROW_PACKAGES emptied —
      each repo gates its own packages.

## Phase 6 — `native/`

- [x] `native/modules/exec-cranelift` ← `plugin-engine-exec-cranelift`
      (feature-gated).
      **Done:** subtree-split import; `kit-canonical`'s optional
      `cranelift` dep now pins the native repo; `handl-runtime` +
      `handl-defining-handl` dev-deps follow via git deps + `[patch]`.
      ~95 tests green in `native/` (incl. `standalone_bootstrap`).
- [x] `native/modules/compile-llvm` ← `plugin-compile-llvm`, **blocked
      until `inkwell` is actually enabled** — today it emits nothing.
      **Done anyway:** the move is mechanical — `inkwell` stays an
      optional, default-off feature; `plugin-ext-compile` consumes the
      crate via a pinned git dep. Enabling the backend remains its own
      task.

## Phase 7 — `dot/`, `term/`, `sexp/`, `handl/` (four repos)

- [x] `dot/modules/compile-dot` ← `plugin-compile-dot` (target `dot`).
- [x] `term/modules/compile-term` ← `plugin-compile-term` (target `term`).
- [x] `sexp/modules/compile-sexp` ← `plugin-compile-sexp` (target `sexp`).
- [x] `handl/modules/compile-handl` ← `plugin-compile-handl` (target
      `handl`). **Repo naming:** the platform repo is renamed
      `platform/` so `handl/` unambiguously means the language target.
      **Done (all four):** subtree-split imports into the four target
      repos; `plugin-ext-compile` consumes them via pinned git deps +
      per-repo `[patch]` tables. `compile-handl`'s roundtrip test
      fixtures (`playground/composition-{hughes,emidio}.hnl`) were
      vendored into `handl/modules/compile-handl/tests/fixtures/` —
      `playground/` is its own repo now.

## Phase 8 — hosts out (last: they sit at the top of the fan)

- [x] `cli/` repo ← `host-cli` + `plugin-ext-cli` (its only consumer).
      The `hnl` binary's target set = its git-dep list.
      **Done:** `handl-lang/cli` holds `crates/{host-cli,plugin-ext-cli}`
      (subtree-split history grafted under `crates/`). ~40 platform
      crates are git deps; `kit-canonical` (default features) and
      `handl-kit-data-canonical` come from the sibling repos; the root
      `[patch]` tables resolve the whole closure from sibling checkouts
      (zero git-sourced packages in the lockfile). The ADR's
      `plugin-ext-shell` does not exist in the workspace — dropped.
      `scripts/dogfood-gate.sh` builds `hnl` from `../cli`.
- [x] `lsp/` repo ← `host-lsp` + `plugin-lsp` + `plugin-lsp-std` +
      `plugin-ext-lsp`.
      **Done:** `handl-lang/lsp` holds the four crates under `crates/`
      (subtree-split history). `host-lsp` keeps its
      `default-features = false` git dep on `kit-canonical`; `plugin-lsp`
      `plugin-lsp-std` keep their `lang` deps (`kit-data-canonical`) —
      the intended lsp → lang/kit-canonical direction. `host-lsp` now
      declares `serde` with `derive` explicitly (platform had it via
      feature unification).
- [x] `host-tooling` **stays in platform** — the shared contract both
      host repos depend on.

## Phase 9 — `meta/` (anytime; zero-consumer leaves)

- [x] `meta/crates/defining-handl` ← `handl-defining-handl`.
- [x] `meta/crates/typecheck-nbe` ← `plugin-engine-typecheck-nbe`.
      **Done:** subtree-split imports; both are zero-consumer leaves so
      no consumer rewiring was needed. `defining-handl`'s manifest now
      git-deps platform + lang + stdlib + native + kit-canonical with
      full `[patch]` coverage. **Note:** 18 `typecheck-nbe` tests fail
      identically on main before the move (pre-existing "unbound
      variable true/false" drift in the leaf) — carried over, not
      introduced.

## Phase 10 — `ffi/`

- [x] `ffi/modules/ffi` ← `plugin-ffi` (cranelift + libloading leave
      platform's lock entirely). `lang/` git-depends on `ffi/` since
      `kit-canonical` is its only normal consumer.
      **Done:** `handl-lang/ffi` holds `modules/ffi` (subtree-split
      history; 53 tests green). Consumers rewired: `native`'s
      exec-cranelift and `kit-canonical`'s optional `ffi` feature now
      pin the ffi repo (their revs bumped here); `meta` repointed its
      dep + patch. `cli`'s patch table was refreshed for Phases 1–10
      (stale `../platform/crates/*` paths repointed + per-repo blocks
      for the new git sources); `lsp` needs none (its closure never
      reaches the moved crates). No platform member deps on
      `handl-plugin-ffi`, so the workspace dep entry went away; a
      `[patch.".../ffi"]` block covers the dev-dep path through
      kit-canonical.

## Phase 11 — `lang/` + `kit-canonical/` (needs 0.8 first)

- [x] `lang/` ← `handl-kit-data-canonical` + canonical `data/*.yaml` +
      the `handl-packages/` dirs pinned to it — the language as data.
      **Done:** sibling repo `handl-lang/lang` holds
      `crates/handl-kit-data-canonical` (subtree-split history), its
      sole-consumer packages `handl-packages/{query,command,event}`
      (also subtree-split), and a *copy* of the six
      `examples/handl/*.hnl` files `tests/loop_engines.rs` embeds (the
      originals stay in platform — they are pinned by staying crates'
      tests too). Its platform deps are git deps on
      `handl-lang/platform`; the workspace-root `[patch]` table resolves
      them from `../platform` for local development.
- [x] `kit-canonical/crates/kit-canonical` ← `kit-canonical` — the
      assembled canonical kit; git-deps on `lang/`, `stdlib/`, `ffi/`
      and platform engines. `cli/`/`lsp/` consume `kit-canonical/` for
      their default language.
      **Done:** sibling repo `handl-lang/kit-canonical` holds
      `crates/kit-canonical` (subtree-split history) plus
      `handl-packages/`: moved sole-consumer dirs (`comb`, `ffi`,
      `noise-c`, `noise-stub`, `os-c`, `os-stub`, `perlin`, `rand-stub`,
      `simplex`, `test`, `type-handl`, `uuid-c`, `uuid-stub`,
      `playground/rand-stub-test.hnl`) and *copies* of dirs still pinned
      by platform crates (`rand-c`, `time-c`, `intl-c`, `unicode-c`,
      `time`, `type`, `generated/icu-{c,uc}-linux-x86_64` — reconcile in
      Phase 3 when `stdlib/` takes ownership). Shared pins applied the
      0.7 ownership rule instead of duplicating: `plugin-lib-stdlib`
      exports `GREET_HNL`/`TRANSFORM_{MAP,FILTER,REDUCE}_HNL` and
      `plugin-lib-handl` exports `DEDENT_HNL`; the kit consumes the
      consts. Platform consumers (`host-cli`, `host-lsp`,
      `plugin-lsp-std`, `handl-defining-handl` + dev-deps) now take the
      crates via git deps, unified with in-tree crates by the root
      `[patch.".../platform"]` block; a `url.insteadOf` +
      `CARGO_NET_GIT_FETCH_WITH_CLI` recipe resolves the git sources
      locally until the remotes are pushed (recipe in the root
      `Cargo.toml` comment).
- [x] **Not moved:** `handl-kit-data` (generic kit types — consumed by
      staying `plugin-ext-kit`), `handl-kit-arrow-lower` (linked by
      `handl-runtime`; moving it would cycle platform↔lang),
      `handl-emitters` (generic fragment emitter, consumed by staying
      `handl-kit-data`), `plugin-ext-refinement` (consumed by
      `plugin-engine-test`).

## Phase 12 — `docs/` (independent, anytime)

- [x] `docs/` repo ← platform `docs/` (VitePress). **Done** (ADR-0129 5.3).
      The platform `adrs/` top-level dir is gone: its 9 ADRs were distributed
      to the repos that own them (`extension-vscode` 0001–0004/0006, `cli`
      0005, `web` 0007, `lsp` 0008, `platform` 0009, `lang` the n-dim
      calculus ADR renumbered 0146) and live on each repo's `project` branch
      under `project/adrs/`. The hub publishes the aggregate instead of
      holding copies.
- [ ] Per-repo generated reference: each repo publishes rustdoc + its
      `.hnl` module surface; `docs/` aggregates them into one site.
      **Deferred — pipeline work, not platform work.** Needs per-repo CI
      publishing rustdoc + module-surface dumps into the `docs` repo;
      `platform/scripts/gen-repo-docs.py` exists as the seed. Documented
      deferral: no user-visible behavior depends on it.

## Deferred

- `plugin-loader-native` dylib registration; `graphql/`, `pg/`, `sqlite/`
  repos once a first module exists.

## Resolved

- **`FragmentSource::DataFile` (ADR-0128 §Deferred) — implemented.**
  `handl-kit-data` now reads a serialized `Language` from a data file
  (`FragmentSource::DataFile`; parser chosen by extension — JSON / YAML /
  TOML) and loads a kit manifest from a file or inline text
  (`DataKitManifest::load` / `DataKitManifest::load_from_text`). Read and
  parse failures name the file. A module repo can therefore ship fragment
  data and kit manifests *as data* instead of `include_str!`-pinning them
  into another crate.
- **The module-name import fallback the `.hnl` moves need — already
  implemented; no platform work required.** Verified by
  `crates/handl-runtime/tests/bare_module_import.rs` (2 tests): a module
  file containing `from "libc" import socket` resolves, and applying the
  module's exported arrow reaches `libc.socket` through the module chain.
  - File-loader path: `ModuleLoader::strip_module_decl_lines`
    (`handl-runtime/src/lib.rs:1805`) lowers a non-file-resolving
    `from "<name>" import <fn>` to `<fn> := "<name>.<fn>"` via
    `module_prep::host_import_bindings` (`module_prep.rs:494`); the string
    callee dispatches through `ModuleLookup`.
  - `HandlModule` path: `DependencyImportResolver`
    (`plugin-lib-handl/src/lib.rs:194`) is auto-wired when a `HandlModule`
    has deps (`lib.rs:1449`), and `TableImportResolver`
    (`stdlib/.../packages.rs:1142`) does the same over the package table.
  - So the only requirements are **registration + spec rewrite**, below.

## The `.hnl` move — landed via `[dependencies]` name resolution

The prescribed module-registration route (`c/modules/lib-c`, wire-name
additions) was superseded by the lighter, already-built Phase-9.5 seam:
`.handl.toml` `[dependencies]` maps an import name to a package directory
and `ModuleLoader` resolves first-segment matches into it — so the specs
became `from "libc/socket"` / `from "ws-c"` / `from "concurrent-epoll"`
and the repo boundary is a manifest dep. `c/` and `web/` each hold a
`handl-packages/` subtree split pruned to their keep sets plus a manifest;
the ws-c↔concurrent-epoll cross-repo cycle resolves both ways via the two
manifests (`c` self-declares its own names because dep files loaded under
its context carry name specs). `import_preamble` (`plugin-engine-test`)
and `host-cli`'s decl-stripping/splice-seeding gained dep awareness; the
tooling kit is `Kit::discover`-based. Regression:
`handl-runtime/tests/dependency_import.rs`.

Residual, if module-chain registration is ever wanted on top: dep names
take precedence over module lowering by design, so registering `libc` et
al. as modules remains compatible. `math-c` and `generated/icu{,-uc}*`
stay in platform (uncoupled / stdlib-owned artifacts); per-repo dogfood
gates for the moved packages are a `c/`/`web/` follow-up.



**Finding 1 — the generated dirs are build artifacts, not source.** Every
`handl-packages/generated/<lib>/` *port* and `<lib>-<backend>-<target>/`
*adapter* is produced by `hnl sdkgen` from a `*.sdkgen.yaml` config plus spec
inputs (`*.idl`, `curl-spec/curl.h`). The `c` backends already live in `c/`
and the `webidl`/js ones in `web/`. So the moved trees are **regenerated in
the target repo**, not hand-rewritten:
`hnl sdkgen c handl-packages/generated/libc.sdkgen.yaml --output c/handl-packages/generated`.

**Finding 2 — the web and `-c` blocks are one unit.** `fetch-c` and `ws-c`
import `../concurrent-epoll/concurrent-epoll`, and `concurrent-epoll` imports
`../generated/libc-c-*`, `../generated/curl-c-*`, `../tls-libssl`, `../ws-c`.
That is a cycle across the two "blocks" — they cannot move independently.
(They *decouple* once the specs are names: `c/`'s modules import `"concurrent"`
and `web/` registers it.)

**Blocker:** the move needs `git subtree split`, which needs a clean tree —
`platform` currently has uncommitted WIP (`plugin-lib-handl/src/transform.rs`,
`handl-kit-arrow-lower/src/lib.rs`, `handl-packages/desugar/`, …). Commit or
stash first.

### Inventory (module name ← source spec)

| source spec (today) | module name | owning repo |
|---|---|---|
| `../generated/libc-c-linux-x86_64/<f>` | `libc` (18 adapter files: `socket`, `unistd`, `epoll`, `fcntl`, `errno`, `time`, `inet`, `un`, `stdio`, `stdlib`, `random`, …) | `c/` |
| `../generated/curl-c-linux-x86_64/curl` | `curl` | `c/` |
| `../generated/libz-c-linux-x86_64/zlib` | `libz` | `c/` |
| `../generated/libssl-c-linux-x86_64/ssl` | `libssl` | `c/` |
| `../tls-libssl/tls-libssl` | `tls` | `c/` |
| `../compress` | `compress` | `stdlib` (platform fallback today) |
| `../math/<f>` (`Acosh`, …) | `math` | `stdlib` (platform fallback today) |
| `../ws-c/ws-c` | `wsc` | `c/` |
| `../ws-whatwg-c/http` | `ws_http` | `web/` |
| `../ws_frame` / `../ws_frame-handl/frame` | `ws_frame` | `web/` |
| already bare: `type`, `bytes`, `str`, `math`, `option`, `char`, `result` | — | chain |

Names must be **single-segment** — `module_prep::is_module_name_spec` rejects
any spec containing `/`, so `libc/socket` is not addressable; register the
`libc` module (all 18 adapters behind one name) or per-file `libc_socket`
style names.

### Sequence

1. Commit/stash the WIP (unblocks subtree split).
2. `c/`: `git subtree split` the `-c` packages + `generated/{libc,curl,libz,libssl,*-c-linux-x86_64}` + the four `*.sdkgen.yaml` + `curl-spec/`; add `c/modules/lib-c` registering them by name; regenerate the generated trees there.
3. `web/`: same for the web block + 15 `generated/*-webidl-browser/` + the webidl configs; add `web/modules/lib-web`.
4. Rewrite specs to names (the table above); register the names in
   `kit-canonical::canonical_wire_names()`.
5. git deps + `[patch]`; prune `dogfood-gate.sh` `ARROW_PACKAGES` (163-177);
   flip the ADR-0128 Phase 2 / Phase 5 statuses.
