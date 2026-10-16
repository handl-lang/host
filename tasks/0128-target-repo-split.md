# Task 0128 — Target-repo split (ADR-0128)

Split `platform` into the reference platform plus target/ecosystem repos.
Repo = target; module = one implementation of one port; modules register
named targets. Resolution: git deps + `[patch]`, local-only. See
`project/adrs/0128-target-repos.md` for the full extraction list.

## Phase 0 — platform prerequisites (no new repos)

- [ ] **0.1 `SdkgenTarget` descriptor + registry** in `plugin-sdkgen`:
      `name`, flags, config loader, backend, emitter, doc source.
      `plugin-ext-sdkgen` collapses to a lookup over it. Delete the
      `kind != "c" && …` gate and the per-target `match` arms.
- [ ] **0.2 Move the Python spec path** (205 lines: `PythonSpec`,
      `load_python_spec`, `generate_python`, `build_python_hnl`,
      `build_code_assignment`) out of `plugin-ext-sdkgen` — it goes to
      `python/modules/sdkgen-python` in Phase 1.
- [ ] **0.3 `CompileDispatcher` → target registry** in `plugin-compile`:
      no static `register(Box::new(JsCompiler))`; targets opt in via a
      feature-gated registry on the host's manifest.
- [ ] **0.4 `library_registry.rs` → data-driven**: kit YAML `modules:`
      resolves names through a registry the kit supplies, not a static
      `reg.register` table.
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
- [ ] **0.7 `pub const` re-export rule**: each `include_str!`ed package
      gets exactly one owner; other consumers take `pub const`. Apply to
      `hello`, `transform`, `stats` first (each pinned by ≥2 crates).
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
- [ ] **0.10 Applier audit**: `plugin-applier-bootstrap` has zero
      consumers — `BootstrapPlugin` is never instantiated. It stays in
      platform (it's the ADR-0036 host-composition seam, and `cli/`
      will need it to assemble kits), but decide: **wire it** (make
      `host-cli` compose via `BootstrapPlugin` as designed) or
      **document superseded** (host-cli's inline path won). Do not
      delete blindly.

Phase-0 verification: `cargo check --workspace` green; `hnl sdkgen
c|webidl|python` and `hnl compile js|llvm|dot|term|sexp` behavior
unchanged (all still statically linked, just through registries).

## Phase 1 — pilot: `python/`

- [ ] New repo `python`: workspace + `modules/sdkgen-python` crate
      (`Emitter` impl + the moved 205 lines) depending on
      `handl-plugin-sdkgen` via git dep; `[patch]` to sibling path for
      local dev.
- [ ] `hnl sdkgen python` resolves `python` → target record
      `(backend: c, emitter: python)`; cross-repo dependency on `c/`'s
      module is expressed at the *target* level.
- [ ] `dev-link.sh` (or superrepo) generates the `[patch]` block across
      sibling checkouts; CI uses the git dep only.
- [ ] NB: `python` works in Phase 1 because `backend: c` still resolves
      in-platform — Phase 2 re-points it to `c/`'s module. That
      re-resolution is the cross-repo composition proof, not an
      accident.
- [ ] Prove: `python/` CI builds only its own graph; `platform/` tests
      green without the emitter.

This phase validates the whole template on a 1-crate port surface with
0 external deps and simultaneously shrinks platform by 205 lines.

## Phase 2 — `c/`

- [ ] `c/modules/sdkgen-c` ← `plugin-sdkgen-backend-c` (`clang-sys` leaves
      the platform lock/build).
- [ ] Decide the C-ABI package block: `-c` packages and 6
      `generated/*-c-linux-x86_64/` dirs follow or stay (item 7).

## Phase 3 — `stdlib/`

- [ ] `plugin-lib-stdlib` + its 31 `include_str!`ed packages move to
      `stdlib/` (`crates/plugin-lib-stdlib` + `packages/`).
- [ ] The other plugin-lib content crates move as modules:
      `stdlib/modules/{kv,kv-hashmap,crypto,encoding,parse,serialize,
      char-native,lib-lang-{lex,parse,eval,typecheck,compile,
      run-compiled}}` — 14 modules total. **Exceptions — machinery stays
      in platform:** `plugin-lib-handl` (`.hnl` loader), `plugin-lib-tag`
      (host primitive), `plugin-lib-scheduler` (test engine).
- [ ] `kit-canonical`, `plugin-ext-kit`, `plugin-engine-test` and
      `plugin-engine-typecheck-handl` git-depend on `stdlib/` — the
      deliberate inversion. Wire via the same git-dep + `[patch]`
      mechanism.

## Phase 4 — `otlp/`

- [ ] `otlp/modules/{lib-otlp,lib-otlp-std}` ← `plugin-lib-otlp` +
      `-otlp-std` + `handl-packages/{otlp,otlp_std}` +
      `kits/server.yaml` fragment + `examples/otlp`.

## Phase 5 — `web/`

- [ ] `web/modules/sdkgen-webidl` ← `plugin-sdkgen-backend-webidl`
      (target `webidl`; CLI keeps the name).
- [ ] `web/modules/compile-js` ← `plugin-compile-js` (target `js`).
- [ ] `.hnl` web block + 13 `generated/*-webidl-browser/` dirs +
      `editor-support/` + `playground/`.

## Phase 6 — `native/`

- [ ] `native/modules/exec-cranelift` ← `plugin-engine-exec-cranelift`
      (feature-gated).
- [ ] `native/modules/compile-llvm` ← `plugin-compile-llvm`, **blocked
      until `inkwell` is actually enabled** — today it emits nothing.

## Phase 7 — `dot/`, `term/`, `sexp/`, `handl/` (four repos)

- [ ] `dot/modules/compile-dot` ← `plugin-compile-dot` (target `dot`).
- [ ] `term/modules/compile-term` ← `plugin-compile-term` (target `term`).
- [ ] `sexp/modules/compile-sexp` ← `plugin-compile-sexp` (target `sexp`).
- [ ] `handl/modules/compile-handl` ← `plugin-compile-handl` (target
      `handl`). **Repo naming:** the platform repo is renamed
      `platform/` so `handl/` unambiguously means the language target.

## Phase 8 — hosts out (last: they sit at the top of the fan)

- [ ] `cli/` repo ← `host-cli` + `plugin-ext-cli` (its only consumer).
      The `hnl` binary's target set = its git-dep list.
- [ ] `lsp/` repo ← `host-lsp` + `plugin-lsp` + `plugin-lsp-std` +
      `plugin-ext-lsp`.
- [ ] `host-tooling` **stays in platform** — the shared contract both
      host repos depend on.

## Phase 9 — `meta/` (anytime; zero-consumer leaves)

- [ ] `meta/crates/defining-handl` ← `handl-defining-handl`.
- [ ] `meta/crates/typecheck-nbe` ← `plugin-engine-typecheck-nbe`.

## Phase 10 — `ffi/`

- [ ] `ffi/modules/ffi` ← `plugin-ffi` (cranelift + libloading leave
      platform's lock entirely). `lang/` git-depends on `ffi/` since
      `kit-canonical` is its only normal consumer.

## Phase 11 — `lang/` + `kit-canonical/` (needs 0.8 first)

- [ ] `lang/` ← `handl-kit-data-canonical` + canonical `data/*.yaml` +
      the `handl-packages/` dirs pinned to it — the language as data.
- [ ] `kit-canonical/crates/kit-canonical` ← `kit-canonical` — the
      assembled canonical kit; git-deps on `lang/`, `stdlib/`, `ffi/`
      and platform engines. `cli/`/`lsp/` consume `kit-canonical/` for
      their default language.
- [ ] **Not moved:** `handl-kit-data` (generic kit types — consumed by
      staying `plugin-ext-kit`), `handl-kit-arrow-lower` (linked by
      `handl-runtime`; moving it would cycle platform↔lang),
      `handl-emitters` (generic fragment emitter, consumed by staying
      `handl-kit-data`), `plugin-ext-refinement` (consumed by
      `plugin-engine-test`).

## Phase 12 — `docs/` (independent, anytime)

- [ ] `docs/` repo ← platform `docs/` (VitePress) + sibling
      `handl-lang/docs/` top-level ADRs.
- [ ] Per-repo generated reference: each repo publishes rustdoc + its
      `.hnl` module surface; `docs/` aggregates them into one site.

## Deferred

- `plugin-loader-native` dylib registration; `FragmentSource::DataFile`
  module-chain seam for crate-unowned `.hnl` packages; `graphql/`,
  `pg/`, `sqlite/` repos once a first module exists.
