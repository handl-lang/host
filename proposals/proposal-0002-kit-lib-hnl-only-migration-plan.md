+++
id = "P-002"
title = "kit-*/lib-* .hnl-only migration plan"
status = "open"
created = "2026-11-20"
owner = "devin"
priority = "med"
labels = ["hnl", "refactor"]
epic = "weekend-2026-11-20"
+++

+++
id = "P-002"
title = "kit-*/lib-* → .hnl-only migration plan (crate-model decision)"
status = "open"
created = "2026-11-20"
owner = "devin"
priority = "med"
labels = ["refactor", "hnl", "weekend"]
epic = "weekend-2026-11-20"
+++

# `kit-*`/`lib-*` → `.hnl`-only: migration plan

Target state (user directive 2026-11-20): `lib-*` and `kit-*` are pure Handl
packages — **no Rust, no Python**. Interop/bindings via `tool-sdkgen-*`
(ex-sdkgen-*) where native access is needed. `host` stays distribution-only.
`kernel` is frozen.

## Current crate models in `lib-*` (audit 2026-11-20)

1. **Pure `.hnl`** (~68) — already target state; `pkg.hnl` + `src/*.hnl`.
2. **Const-only `handl-lib-*` crates** (~30) — Rust crates that only embed
   `*_HNL` source consts for host crates; the package surface is still `.hnl`.
   Migration = move const embedding into the `.hnl` package mechanism
   (`plugin-lib-stdlib` build already copies `pkg.hnl`+`src/**`), retire crate.
3. **Native `handl-plugin-lib-*` crates** (15) — real Rust implementations
   (`lib-{bytes,cmp,error,mood,obj}` kernel-substrate ops + codec/backend
   crates). Options per repo: (a) port to `.hnl` over `handl-plugin-ffi`,
   (b) keep native but relocate out of `lib-*` (e.g. `kernel` substrate ops
   stay put — kernel frozen; codec backends become `tool-*`/`host` internals),
   (c) `tool-sdkgen-*` bindings where it's FFI surface.

## Broken/edge cases to fix during migration (from audit)

- `ci.sh` runs `cargo test` with no `Cargo.toml`: `lib-{arrow-iter,datetime,
  encoding,stats}` — gate broken today.
- Unresolvable `pkg.hnl` modules: `lib-comb`, `lib-os-c`, `lib-os-stub`,
  `lib-ip` (partial — flat `src/*.hnl` vs `src/<name>/index.hnl` convention),
  `lib-curl`, `lib-libc`, `lib-libz` (no `.hnl` at all — generated bindings
  under gitignored `handl-packages/generated/`).
- Legacy `handl-packages/` layout without `pkg.hnl`: `lib-{command,event,
  query,otlp}` (+`lib-otlp` dual layout).
- Stray `../../lib-*` relative imports: `lib-{os-c,noise-c,unicode-c,ffi}`.
- `lib-math-js`/`lib-mood`-style skeleton repos (`src/` empty).
- ~40 crates declared `MIT OR Apache-2.0` while shipping single MIT LICENSE —
  resolved org-wide by T-114 (single MIT).

## Python `kit-*` (torch stack)

`kit-{carriers,cuda,discovery,host-tensor,kernel-ir,onnx,torch}` are Python —
they do NOT fit `.hnl`-only. Likely relocation out of `kit-*` to product/
research repos (`research-*` or domain repos) — decide per-repo when its
migration comes up; transitional `kit-python.md` template covers them until
then. `host-tensor` is NOT a host (distribution-only rule) — its target is
`lib-*`/`engine-*` territory, undecided.

## Order (on demand, not big-bang)

1. Fix gates: the 4 broken `ci.sh` + 6 unresolvable `pkg.hnl` (cheap, unblocks).
2. Const-only crates → `.hnl` package mechanism (bulk, mechanical).
3. `handl-packages/` legacy repos → `pkg.hnl` + `src/` layout.
4. Native crates: per-repo decision (port / relocate / sdkgen-bind).
5. Python kits: relocation decisions.

Each step is its own commit series per repo; no flag-day.
