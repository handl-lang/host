---
title: Repo visibility is derived from the dep closure, not chosen per-repo
type: adr
date: 2026-10-21
status: accepted
status_note: Accepted
---

# ADR-0150: Repo visibility is derived from the dep closure, not chosen per-repo

## Context

The org splits one-crate-per-repo (ADR-0128), which makes repo
visibility a graph property: a public repo's `git` deps must resolve
anonymously, so every repo in its transitive `cargo metadata` closure
must also be public. Visibility was historically set ad hoc — repos
went public or private without checking who still referenced them,
which left the public `research-mlopt` unable to `cargo build` for
outsiders (its closure reached private repos).

## Decision

**A repo is public iff it sits in the dep closure of a public seed
repo** (`research-mlopt` today). Visibility is *derived*, never chosen
per-repo. Two rules keep the derived set honest:

1. **Edges point private → public.** An adapter or tool binding lives
   in the repo of the tool it serves, never in the library crate —
   otherwise a public library drags private tooling into its closure.
   (`HandlNativeLint` moved from `plugin-lib-handl` to
   `plugin-ext-lint` under this rule.)
2. **The closure is checked, not remembered.**
   `research-mlopt/scripts/check_public_closure.sh` fails if any
   `handl-lang/*` git source in its lockfiles resolves to a private
   repo. Run it after touching dep edges.

### Split applied under this rule

`handl-kit-data-prelude` split into:

- **`handl-kit-prelude-lang`** (public, new repo `kit-prelude-lang`) —
  the eval-path fragment set (`tokens`, `grammar`, `eval`, `types`,
  `ast`, `caps`, `options`, `lexer_rules`, `transforms`, `globals`) +
  `language_from_yaml` / `language_from_dir` / `merge_lang_fragments` /
  `merge_lints` / `merge_lsp`. This is the canonical `Language` minus
  the tooling sections — the dep a lex/parse/eval consumer needs.
- **`handl-kit-data-prelude`** (private) — the composition wrapper:
  composes `kit-prelude-lang`'s language, merges `lints.yaml` +
  `lsp.yaml` on top, keeps `data/kits/*` and the `DataKit` machinery.
  All 77 tests pass unchanged; the composed `Language` is identical.

### Tier model

| tier | contents | visibility |
|---|---|---|
| substrate | `kernel`, `engine` bases, `engine-*-impl` that are load-bearing (`-pattern`, `-pratt`, `-tree-walk`, `-bytecode`, `-rewrite`, `-typecheck`), `.hnl` eval deps (`lib-desugar`, `lib-loader`, `lib-str-handl`, `lib-char{,-native}`), `host`, `host-tensor`, `kit-prelude-lang` | public at launch |
| research | `research-mlopt`, `egraph`, `discovery` | public |
| surface | `docs`, `slidev-theme` | public |
| tooling | `engine-lint`, `engine-fmt`, `engine-test`, `ext-*`, `host-cli`, `host-lsp`, `kit-*`, remaining `lib-*` ports | private until done |

## Consequences

- `engine-lint` leaves the mlopt closure permanently (adapter moved).
- `engine-exec-bytecode` / `engine-typecheck` stay in the closure —
  the `.hnl` loader runs on bytecode, and `handl-runtime` re-exports
  the `TypecheckEngine` interface. Both are substrate, not severable;
  they flip public at launch.
- Eight private repos remain in the closure today
  (`lib-desugar`, `lib-loader`, `lib-str-handl`, `lib-char`,
  `lib-char-native`, `engine-transform-rewrite`,
  `engine-exec-bytecode`, `engine-typecheck`) — a single visibility
  flip each, no code change.
- Public repos' `cargo test` may still pull private dev-deps; the
  public guarantee is `cargo build`.
