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

**P2 — pilot.** `lib-pred`: delete `Cargo.toml` + `src/lib.rs`; `engine-test`
loads `pred` via the loader. Zero `include_str!` in the chain; its suite green.

**P3 — sweep.** Remove the remaining 87 shells, one repo per commit, each with
its consumer moved. Gate: `hnl-gate.sh` + `verify-all.sh` green throughout.

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
