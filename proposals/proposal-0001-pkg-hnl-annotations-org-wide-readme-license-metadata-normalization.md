+++
id = "P-001"
title = "pkg.hnl annotations + org-wide README/LICENSE/metadata normalization"
status = "open"
created = "2026-11-20"
owner = "devin"
priority = "high"
labels = ["metadata", "readme"]
+++

# pkg.hnl package annotations + org-wide README/LICENSE/metadata normalization

Two coupled changes across all `handl-lang/*` repos:

## 1. `pkg.hnl` annotation schema

`pkg.hnl` is currently functional-only (`{ modules, deps, entry }`) and is read
by **textual substring scanners** (`handl_platform::pkg_modules` /
`pkg_entry` / `pkg_dep_specs` in `host/crates/platform/src/config.rs`) —
`str::find("modules"|"deps"|"entry")` on comment-stripped source, first match
wins, not string-aware for the key.

### Schema

Flat top-level keys; functional keys unchanged:

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

| Key | Type | Required | Notes |
|---|---|---|---|
| `modules` | `[str]` | yes | unchanged |
| `deps` | `{name: spec}` | no | unchanged |
| `entry` | `str` | no | unchanged |
| `name` | `str` | no | matters for `handl-packages/<pkg>/` multi-package repos |
| `description` | `str` | yes | one line; same text as README lead |
| `license` | `str` | yes | `"MIT"` |
| `keywords` | `[str]` | no | |
| `homepage` | `str` | no | default `github.com/handl-lang/<repo>` |

### Safety constraint

Annotation string values must not contain the substrings `deps`, `modules`,
or `entry` (comments are already stripped and exempt). Enforced by a gate
check.

### Parser hardening (host repo, prerequisite)

Update the three scanners in `config.rs` to require `key` followed by
optional whitespace + `:` before scanning — removes substring
false-positives except a literal `deps:`/`modules:`/`entry:` inside a
string, which the gate check bans. ~30 lines + tests.

`handl-packages/<pkg>/pkg.hnl` uses the same schema (`name` distinguishes
packages). `crates/<crate>/src/*.hnl` substrate packages have no `pkg.hnl`;
doc comments remain their annotation surface.

## 2. Org-wide README / LICENSE / metadata normalization

Canonical templates live in `handl-lang/workspace/templates/README/`
(`rust-crate`, `rust-workspace`, `handl-pkg`, `python-pkg`, `fallback`).

Rules: license is always **MIT** (README says `MIT.`; every repo carries an
MIT `LICENSE` file — add where missing); **no provenance** in READMEs (no
ADR citations, split notes, history); `## Documentation` is a bare docs-hub
URL, omitted when no site exists; normalize structure, keep existing prose.

Metadata everywhere:

- `Cargo.toml [package]`: `description`, `license = "MIT"`, `repository`,
  `homepage`/`documentation`, `keywords`, `categories`, `readme` where
  missing.
- `pkg.hnl`: the annotation schema above.
- `package.json`: `name`, `version`, `description`, `license`, `repository`,
  `homepage`.

Scope: ~160 sibling repos, each its own git repo — per-repo commit + push to
`origin/main`. Includes the 14 repos missing READMEs entirely
(`compile-handl`, `kit-{core,ffi,lang,pkg,server,stdlib,test}`, `lib-{ip-c,
json-handl,kv-map,otlp-c,serialize-toml-c,serialize-yaml-c}`).

## Plan

1. Host: harden `pkg_*` scanners + tests; ship first (other repos' gates may
   consume it).
2. `workspace`: push templates repo (needs a GitHub remote).
3. Sweep repos in family order: `lib-*` → `engine-*`/`ext-*`/`compile-*`/
   `sdkgen-*` → `kit-*`/`host-*` → python repos → oddballs. Per repo:
   README normalize-or-create, LICENSE check/add, metadata annotations,
   commit, push.
4. Final sweep: every repo has README + LICENSE + annotated manifests.

Templates approved in session 2026-11-20 (MIT-only, no provenance,
single docs link).
