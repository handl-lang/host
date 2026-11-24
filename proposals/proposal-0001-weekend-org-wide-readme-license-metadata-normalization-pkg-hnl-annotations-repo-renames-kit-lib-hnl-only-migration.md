+++
id = "P-001"
title = "Weekend: org-wide README/LICENSE/metadata normalization, pkg.hnl annotations, repo renames, kit/lib .hnl-only migration"
status = "open"
created = "2026-11-20"
owner = "devin"
priority = "high"
labels = ["metadata", "readme", "weekend"]
epic = "weekend-2026-11-20"
+++

# Weekend push: repo hygiene epic

Approved-in-session scope (2026-11-20). Canonical templates + vendored gate
tooling live in `handl-lang/workspace/templates/` (`README/{family}.md`,
`xtask/`, `gate.py`).

## 1. `pkg.hnl` annotation schema

`pkg.hnl` is functional-only (`{ modules, deps, entry }`) and read by textual
substring scanners (`handl_platform::pkg_modules` / `pkg_entry` /
`pkg_dep_specs` in `host/crates/platform/src/config.rs`) — `str::find` on
comment-stripped source, first match wins.

Schema (flat top-level keys; functional keys unchanged):

```hnl
{
  modules: ["hello"],
  deps: { math: "branch:main" },        -- optional
  entry: "src/index.hnl",               -- optional
  name: "hello",                        -- optional; default = repo minus prefix
  description: "the `greet` arrow",    -- required
  license: "MIT",                       -- required
  keywords: ["greet", "hello"],         -- optional
  homepage: "https://github.com/handl-lang/lib-hello",  -- optional
}
```

Constraint: annotation string values must not contain `deps`, `modules`, or
`entry` substrings (comments exempt). Enforced by gate check.

Prerequisite: harden the three scanners in `config.rs` to require `key` +
optional-ws + `:` (~30 lines + tests) — host repo, ship first.

## 2. README / LICENSE / metadata sweep (all ~160 repos)

- Templates per prefix family: `engine`, `tool-compile`, `tool`,
  `tool-sdkgen`, `lib`, `lib-rust` (transitional), `kit`, `kit-rust`/
  `kit-python` (transitional), `host`, `pitch`, `python`, `bare`.
- Rules: license always **MIT** + `LICENSE` file in every repo; **no
  provenance** in READMEs; `## Documentation` = bare docs-hub URL or omitted;
  normalize structure, keep prose.
- Gate tooling: vendor `xtask/` + `.cargo` alias into Rust repos
  (`cargo xtask gate`); vendor `gate.py` → `scripts/gate.py` into python
  repos (`uv run scripts/gate.py`).
- Manifest metadata: `Cargo.toml [package]` full fields; `pkg.hnl`
  annotations; `package.json` fields.
- 14 repos missing READMEs get them created.

## 3. Repo renames (local prep now; GH ops deferred)

- `sdkgen-*` → `tool-sdkgen-*` (5)
- `compile-*` → `tool-compile-*` (8)
- `slidev-theme` → `slidev-theme-handl`
- `mlopt` → `research-mlopt`
- `host-tensor` → NOT a host (host = distribution only); target
  `lib-*`/`engine-*` — decision pending, tensor likely lib-adjacent
- Templates already reflect target names (`tool-compile.md`,
  `tool-sdkgen.md`, `tool.md`, `pitch.md`).

## 4. `kit-*`/`lib-*` → `.hnl`-only target

Target state: `lib-*` and `kit-*` are pure Handl packages — no Rust, no
Python. Interop/bindings via `tool-sdkgen-*` where needed. This is a
planned-on-demand refactor, not big-bang: transitional `lib-rust.md`,
`kit-rust.md`, `kit-python.md` templates stay until each repo migrates.
Python kit stack (torch/cuda/onnx/carriers/discovery/host-tensor/kernel-ir)
likely relocates out of `kit-*` — decide per-repo during migration.

## 5. Constraints

- `kernel` is frozen — no changes without a great reason.
- **Local-first**: do all local work (commits, vendored tooling, README
  edits) without stopping; anything needing GH permission (repo creation,
  renames, pushes to *new* remotes) is batched into a deferred publishing
  step for when the user returns. Pushes to *existing* origins were already
  approved.

## Audit input (2026-11-20, reference not truth)

`handl-lang/workspace/audit-2026-11-20.md` — lib-* is the worst family (~6
architectures: kernel-substrate Rust crates in lib- clothing, broken ci.sh
×4, unresolvable pkg.hnl ×6, legacy handl-packages/ ×4+, three crate
models). Top-fix ordering feeds the sweep: broken gates first, then
pkg.hnl resolvability, then the lib crate-model decision (resolved by the
.hnl-only target), license metadata, handl-packages/ migration.

## Execution order

1. host: `config.rs` scanner hardening + tests.
2. `workspace`: templates repo — create GH remote (deferred publish).
3. Per-family sweeps: `lib-*` → `engine-*`/`ext-*`/`compile-*`/`sdkgen-*` →
   `kit-*`/`host-*` → python → `pitch-*`/`bare`.
4. Rename prep (local branches/dirs + README/manifest name updates).
5. Deferred publishing batch (needs user go): GH creates/renames/pushes.
