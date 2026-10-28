# ADR-0160: `lib-*` repos are language packages, not crates

## Status

Proposed — 2026-10-28. Owners: every `lib-*` repo, `host` (loader),
`automation` (gates).

## Context

The `lib-*` split (ADR-0128) made each stdlib port a repo. The contract is a
Handl package: `pkg.hnl` (`{ deps, modules, entry? }`) plus `.hnl` sources. The
intent was explicit — "Rust in stdlib is transitional" (stdlib migration plan
§1), and the target layout is:

```text
lib-<port>/
  pkg.hnl
  src/*.hnl
  README.md
```

Reality today (2026-10-28 audit of all 113 repos):

| what | count |
|---|---|
| repos | 113 |
| with a Rust shell (`src/lib.rs` doing `include_str!`) | **88** |
| with `pkg.hnl` | 92 |
| with `ci.sh` | 93 |
| with a README | 107 |
| with a `LICENSE` | **5** |

Two problems.

1. **The shell exists only for Rust consumers.** `include_str!` is the only way
   a Rust crate can carry foreign source; Cargo has no data dependency. So every
   repo that a Rust crate consumes grows a 7–376 line shim that is pure
   transport (`lib-pred/src/lib.rs` is seven lines: `ROOT` + `PRED_HNL`).
2. **The packaging is inconsistent.** License declarations disagree
   (`MIT` vs `MIT OR Apache-2.0`, and 108 repos ship no license text at all),
   CI is missing in 20 repos, six have no README, and 21 have no `pkg.hnl` —
   i.e. they cannot be loaded as packages at all.

The blocker for (1) is not the repos, it is the loader: `pkg.hnl` `deps` resolve
to **sibling checkouts** today. Until they resolve over git, a Rust consumer has
no way to obtain the `.hnl` except by embedding it.

## Decision

**Treat every `lib-*` repo as a Handl package with a fixed shape, and delete the
Rust shells as the loader learns to fetch deps.**

1. **Repo shape is fixed** (all 113):
   `pkg.hnl` · `src/**/*.hnl` · `README.md` · `LICENSE` (MIT) · `ci.sh` ·
   `bupha.toml`, and no `.github/`.

   **The 89 repos whose payload is `.hnl` carry no `Cargo.toml` and no
   `src/lib.rs`.** The 24 whose *implementation* is Rust — the `-native`, `-c`,
   `-js` backends and `lib-lang-*` substrate adapters — keep their crate: for
   them the crate is the port, not transport. The distinction is mechanical:
   a repo with `.hnl` sources is a package; a repo with none is a backend.
2. **License is MIT only**, one `LICENSE` file. `LICENSE-APACHE`/`LICENSE-MIT`
   pairs fold into it; `license = "MIT OR Apache-2.0"` disappears with the
   crates.
3. **CI is bupha-only**: root `ci.sh` (the repo's gate) + a committed
   `bupha.toml` with `runner.kind = "script"`, `cmd = "bash ci.sh"` and
   `status.context = "ci/bupha"`.
4. **The shell is a bug, not a feature.** It is removed in the same change that
   gives the loader git deps — never repo-by-repo, because a repo whose shell is
   gone while the loader cannot fetch it becomes unbuildable for its consumers.
5. **Consumers move to packages**: `engine-test` (and any other Rust consumer)
   loads the module through the loader instead of `include_str!`.

## Migration

**P0 — standardise the packaging (no behaviour change).**
MIT `LICENSE` + `ci.sh` (where missing) + `bupha.toml` + README sections in all
113 repos. `pkg.hnl` is added where a repo is missing it, mirroring its module
name. Nothing is deleted; every repo stays buildable.

**P1 — loader resolves `deps` over git** (prerequisite for P2, tracked in
`project/BACKLOG.md`):

- `deps: { pred: "branch:main" }` → fetch → cache → `src/<module>/index.hnl`;
- one integration test with no sibling checkout on disk;
- keep the sibling path as the dev fast path.

**P1 finding (2026-10-28): most of the plumbing already exists.**
`handl_platform` already has `package_cache_dir()` (`$HANDL_PKG_DIR` else
`~/.handl/pkg`), `fetch_dependencies()` (`git clone` + checkout of declared deps into
that cache, `HANDL_PKG_FETCH=off` for hermetic runs) and `resolve_pkg_dependencies()`
(`pkg.hnl` `deps` → name→dir map). What is missing is only the **dir→module bridge**: a
`ModuleResolver` that, given a fetched package dir, reads its `pkg.hnl` and builds the
`HandlModule`(s). That is much smaller than "implement a package manager".

**P1 design (what is missing, precisely).** The seam already exists: a package env is
`HandlModule::from_pkg_hnl(src, resolver)` and every declared dep is answered by
`ModuleResolver::resolve(name)` (`host/crates/plugin-lib-handl/src/lib.rs`). Today every
resolver is a static kit table, so an unknown name is a hard error. P1 therefore needs a
resolver that, on a miss:

1. maps `name` → repo `handl-lang/lib-<name>` (then `-handl` / `-c` / `-native` in that
   order) and `spec` → ref (`branch:main`, later `rev:`/`tag:`);
2. fetches into a content-addressed cache (`~/.cache/handl/pkg/<repo>@<sha>`) with the
   `git` CLI — no new crates, no libgit2;
3. reads the fetched `pkg.hnl` → `{ modules, entry? }`, and maps each module to
   `src/<name>/index.hnl` (N modules) or `src/index.hnl` (one);
4. builds each file as a `HandlModule` and wires its imports — `str_handl_modules()` is the
   working template: per-file module, `from "…" import` parsed for edges, siblings first;
   the missing piece there is **field discovery** (the `funcs` list `HandlModule::new`
   needs is a hand-written const today; a package must derive it from the exported record);
5. recurses through the package's own `deps`, with a depth cap + cycle set.

Test plan: pure parts (spec parsing, repo/cache-path derivation, module→path mapping,
field discovery) as unit tests; one integration test that resolves a real repo at a ref
with the sibling checkout renamed away; `HANDL_PKG_OFFLINE=1` must fail loudly, never
silently fall back to a sibling.

**P1 status (2026-10-28, second slice).** Multi-module packages now work: a package's
modules are ordered by their intra-package imports (`order_by_imports`) and each sibling is
wired with `with_dependency_arc`, so `src/<name>/index.hnl` packages bridge like
`str-handl` does. `import_specs` is shared (`pub`) between the hand-written builder and the
bridge, and `PackageResolver::from_manifest_dir(dir)` runs `fetch_dependencies` +
`resolve_pkg_dependencies` and loads whatever the cache holds (fetch failures still degrade
to "unresolved module", never a panic).

**Blocking P2:** `discover_funcs` is a scan, not a parser — braces inside string literals
desync the record walk, and the real `pred` package trips it. The fix is a parser-grade
field scan (the `.hnl` lexer already exists in the kernel); until then P2 cannot rely on
derived fields, so the `lib-pred` shell stays.

**P1 status (2026-10-28, first slice): the bridge landed.** `handl_plugin_lib_handl::pkg` now provides
`package_modules(dir)` (read `pkg.hnl` → `src/…`, one `PackageModuleDef` per module),
`discover_funcs(src)` (field names from `export <name> :=` or the record's top-level keys
— no hand-written `funcs` const), and `PackageResolver` (a `ModuleResolver` over the
platform's name→dir map, skipping cache slots that were never fetched). Seven unit tests
cover discovery, path mapping and resolver behaviour. **Still missing for P2:** multi-module
packages (`src/<name>/index.hnl` + intra-package imports) and wiring
`fetch_dependencies` + `resolve_pkg_dependencies` into the resolver constructor.

**P2 — pilot.** `lib-pred`: delete `Cargo.toml` + `src/lib.rs`; `engine-test`
loads `pred` via the loader. Zero `include_str!` in the chain; its suite green.

**P3 — sweep.** Remove the remaining 87 shells, one repo per commit, each with
its consumer moved. Gate: `hnl-gate.sh` + `verify-all.sh` green throughout.

**P2/P3 status (2026-10-28).** P2 landed: `lib-pred` dropped its `Cargo.toml` +
`src/lib.rs`, and `engine-test` now loads the `pred` module through the bridge
(`package_modules` + derived fields) — 79/79 tests green, `ci: ok`. That fixed the field
discovery properly: brace-stack scan, string/comment aware, counting only single-`:`
record fields (so `name :: …` helpers are skipped) and preferring the last block with
≥2 fields.

P3 is **33 of 88 shells down**: every shell whose crate has no consumer outside its own
repo (32 repos + `lib-pred`) is gone. The remaining **55** each have a Rust consumer, so
each is a two-part change (“strip the shell + move the consumer onto the loader”) and is
being landed one repo at a time.

**P3 target map (2026-10-28).** The 55 remaining shells are not 55 independent jobs:
`host/crates/plugin-lib-stdlib` **alone** consumes 36 of them through a data table
(`packages.rs`: `ModuleSpec { name, hnl: &'static str, funcs, ops, deps }`). Migrating that
table from `include_str!` to package-backed specs
(`pkg::module_from_dir(dir, name)`) retires 36 shells in one change. The rest are small,
single-consumer edges: `kit-prelude` (2), `kit-stdlib` (8 extra), `kit-test` (2),
`kit-core`/`kit-ffi`/`kit-server`/`kit-lang` (1–6 each), `plugin-lib-handl` (4),
`plugin-ext-refinement` (2), `plugin-lib-tag` (1), `handl-runtime` (1), `engine-test` (1),
and five `lib-*`→`lib-*` edges.

## Consequences

- **Good:** a package is `pkg.hnl` + `.hnl`; adding a port needs no Rust; the
  113 repos become uniform (license, CI, README), so a viewer can read any of
  them the same way; and the "Rust is transitional" claim becomes true.
- **Cost:** P1 is loader work in `plugin-lib-handl`, and P2/P3 touch consumers.
  Until P1 lands, the 88 shells stay — deliberately, since removing them early
  breaks every Rust consumer.
- **Risk:** a repo with `pkg.hnl` and no consumer contract (no `axiom` file) is
  just data; the ADR does not require every repo to have a contract, only a
  package.

## Alternatives rejected

- **Feature-gate the shells** — Cargo resolves and *fetches* optional/dev git
  deps, so gating does not make a consumer buildable without the repo
  (verified).
- **Vendor the `.hnl` into each consumer** — duplicates the source and loses the
  version link; it is the same debt with worse provenance.
- **Publish to crates.io** — out of scope by decision; the whole org installs by
  git URL.
