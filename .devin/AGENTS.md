# .devin/AGENTS.md — Agent Notes

**This file is intentionally short.** The previous version documented a
v1 `Scheme` migration state that has been superseded — do not trust any
copy of it floating around. The authoritative agent documentation lives
in the root `AGENTS.md` (plus `.devin/rules/` for scoped rules); read
that first.

## Build and test commands

The root `AGENTS.md` "Build & Test" section is current: `cargo check
--workspace`, nextest (`cargo nextest run --workspace -j 2 --profile
fast`), and the memory constraints for parallel runs. `hnl` itself is
built from the sibling `host-cli` repo.

## Key facts that bite

- The workspace is **v2-only**: the kernel type is `Term` (there is no
  `Scheme` enum anymore — v1 eval path removed; `SchemeVersion` remains
  only as the version marker, and every `Language` targets `v2()`).
- Canonical language data lives in the sibling `kit-prelude-lang` /
  `kit-data-prelude` repos; the `Kit` wrapper is `handl-kit-prelude` in
  the `kit-prelude` repo. There is no `kit-canonical` and no
  `handl-kit-data-canonical`.
- `.hnl` package sources live in the `lib-*` repos (`src/*.hnl` +
  `pkg.hnl`), not `handl-packages/` (that directory only holds
  sdkgen-generated output, if anything).
- Engines/tooling are generic and data-driven off `Language`/`Term` —
  see `.devin/rules/engine-genericity.md` (note: its `Scheme` wording
  predates the v2-only cutover; the substance — dispatch on
  `lang.*` data, never hardcoded kind checks — still holds).
- ADRs are under `project/adrs/`; treat ADR-0040/0044/0052/0081/0092/
  0128/0149/0152 as the load-bearing decisions for current architecture.

## Coverage

`cargo cov*` aliases and `scripts/coverage.sh` are unchanged and still
valid — see the "Coverage" section of the root `AGENTS.md`.
