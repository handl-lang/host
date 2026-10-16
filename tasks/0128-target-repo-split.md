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
- [ ] `hnl sdkgen python` resolves `python` → target record
      `(backend: c, emitter: python)`; cross-repo dependency on `c/`'s
      module is expressed at the *target* level.
      (Not yet — needs Phase 0.1's `SdkgenTarget` registry; the `python`
      target still resolves through `plugin-ext-sdkgen`'s static table,
      now over a git dep.)
- [ ] `dev-link.sh` (or superrepo) generates the `[patch]` block across
      sibling checkouts; CI uses the git dep only.
      (Still hand-maintained, same as stdlib/cli/lsp — the repo CI
      strips `[patch]` and resolves the pinned revs.)
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
- [ ] Decide the C-ABI package block: `-c` packages and 6
      `generated/*-c-linux-x86_64/` dirs follow or stay (item 7).
      (Deferred — ADR-0128 defers this to the item-7 scope decision;
      they stay in platform for now.)

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
- [ ] `.hnl` web block + 13 `generated/*-webidl-browser/` dirs.
      **Deferred with the C-ABI block (item 7):** the web packages are
      entangled with the staying `-c` stack — `concurrent-epoll` /
      `serve-epoll` import `../generated/libc-c-linux-x86_64/*`,
      `../ws-c`, `../tls-libssl`, `../concurrent` by relative path, and
      `fetch-js` imports `../generated/fetch-webidl-browser`. Moving the
      block now would break the platform dogfood gate's curated
      ARROW_PACKAGES and strand cross-repo relative imports.
      (`editor-support/` → `extension-vscode` + `extension-neovim`;
      `playground/` → `playground`; `bench/` → `bench`;
      `examples/handl/` → `handl/examples/` (redesigned into
      `basics/ recursion/ algorithms/ lists/ modules/`); `examples/http/` →
      `web/examples/http/`. All are already gone; `crates/bench` — the
      criterion suite — stays here.)

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

- [ ] `ffi/modules/ffi` ← `plugin-ffi` (cranelift + libloading leave
      platform's lock entirely). `lang/` git-depends on `ffi/` since
      `kit-canonical` is its only normal consumer.

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

- [ ] `docs/` repo ← platform `docs/` (VitePress). **Done** (ADR-0129 5.3).
      The platform `adrs/` top-level dir is gone: its 9 ADRs were distributed
      to the repos that own them (`extension-vscode` 0001–0004/0006, `cli`
      0005, `web` 0007, `lsp` 0008, `platform` 0009, `lang` the n-dim
      calculus ADR renumbered 0146) and live on each repo's `project` branch
      under `project/adrs/`. The hub publishes the aggregate instead of
      holding copies.
- [ ] Per-repo generated reference: each repo publishes rustdoc + its
      `.hnl` module surface; `docs/` aggregates them into one site.

## Deferred

- `plugin-loader-native` dylib registration; `FragmentSource::DataFile`
  module-chain seam for crate-unowned `.hnl` packages; `graphql/`,
  `pg/`, `sqlite/` repos once a first module exists.
