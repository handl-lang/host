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
- [Bytes](./kernel/lib-bytes) — byte-buffer primitives for the `Bytes`
  scalar type.
- [Cmp](./kernel/lib-cmp) — deep equality / total ordering.
- [Combinators](./kernel/lib-combinators) — function combinator
  utilities.
- [Error](./kernel/lib-error) — the error-raising primitive contract.
- [Mood](./kernel/lib-mood) — mood (mode-of-data) operations.
- [Obj](./kernel/lib-obj) — record introspection primitives.
- [Type](./kernel/lib-type) — type coercion, predicates, and utilities.

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

The `lib-*` standard-library packages, the `hnl` toolchain surface, and
the curated `kit-*` kits are not yet released — they produce no pages
and no links on this site.
