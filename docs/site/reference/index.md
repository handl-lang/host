# Reference

The reference is generated per repo (`scripts/gen_reference_all.py`)
from **public** sibling repos only — each section links into
`reference/<repo>/`, one entry per package the repo ships.

## Platform

- [Host concepts](./host-concepts) — traits, types, and the plugin
  lifecycle for composable hosts.
- [Tensor](./host-tensor/host-tensor) — host-side dense n-d tensor
  functions (f64, row-major).

## Kernel substrate (`kernel` repo)

- [Prim](./kernel/lib-prim) — the minimal atom layer for the arrow-based
  substrate.

The rest of the floor libraries dissolved out of the kernel workspace
into their own `lib-*` repos — `lib-bytes`, `lib-cmp`, `lib-comb`
(was `combinators`), `lib-error`, `lib-mood`, `lib-obj`, `lib-type`
(`typeof` merged in), `lib-transform` — and are not yet released (see
*Not here*).

## Research stack (`graphopt`, renamed from `research-mlopt`)

The frozen public surface of the research stack — bound to
`graphopt:docs/public_api.md` (`API_VERSION = "0.1.0"`):

- [The frozen API](/graphopt/reference/frozen-api) — the contracts the repo split
  must not break, tagged per destination repo
  (`eqsat`/`gamekit`/`graphopt`).
- [Discovery scorecard](/graphopt/reference/scorecard) — can the engine derive
  each named optimization, certify it, and beat the registered floor?
  *(generated upstream, vendored)*
- [Player leaderboard](/graphopt/reference/leaderboard) — every registered player
  on every board under one paired protocol. *(generated upstream,
  vendored)*
- [Limits table](/graphopt/reference/limits) — the delivered form vs every
  registered baseline, honest losses included. *(generated upstream,
  vendored)*

## Not here

The `hnl` toolchain surface (the `hnl` CLI and its `ext-*` command
extensions), the curated `kit-*` kits, and most of the `lib-*`
standard library are not yet released — they produce no pages and no
links on this site. The toolchain *engines* (the whole `engine-*`
family) and a handful of `lib-*` packages are public repos; see each
repo's own documentation.
