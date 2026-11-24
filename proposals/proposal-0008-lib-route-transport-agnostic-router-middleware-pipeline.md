+++
id = "P-008"
title = "lib-route: transport-agnostic router + middleware pipeline"
status = "deferred"
created = "2026-11-23"
priority = "med"
labels = ["http", "routing"]
+++

# P-008 — lib-route: transport-agnostic router + middleware pipeline

***Status:** draft · **New repo:** `lib-route` · **Task:** T-136
**Depends on:** nothing (pure `.hnl`, zero deps beyond `str`/`char`/`hash_map`)
**Builds on:** `lib-http-route` v3 pattern compiler, `lib-http-middleware` P-007 protocol

## Motivation

Routing and middleware are not HTTP concepts — they are *dispatch over a
key* and *onion composition over a context record*. Both already exist in
this org and both are more generic than their names admit:

- `lib-http-middleware`'s `pipeline(steps)`/`chain(steps)` fold
  `[mw..., terminal]` into `(ctx) ~> { resp, state }` with **zero HTTP
  imports** — `ctx` is just a record, `next` is just an arrow. Only the
  `req`/`resp` key names and the middleware *bodies* (cors, session,
  auth…) are HTTP.
- `lib-http-route`'s v3 pattern compiler (segment tokens, `:param`,
  `*splat`, `(/optional)` groups, suffix vars) and specificity scoring
  (`seg_score`/`pat_score`/`score_better`) operate on **segment lists**,
  not URLs. The HTTP layer is a thin shell: split `req.path` on "/",
  check method eligibility, merge params into `req`, emit 404/405.

The CLI side proves the demand: `kit-cli-app` curates argv/stdio/posix/
json but has **no subcommand router** — every handl CLI will hand-roll
`match argv { ["deploy", ...] => ... }` trees. Subcommand dispatch is the
same problem as URL routing: nested keys, `:param` positionals
(`deploy :env`), specificity ranking, and middleware-shaped concerns
(global `--json` output wrapping, `--verbose` logging, error formatting).

User framing: *"a router for arrows"* — patterns select handler arrows,
transport supplies the key.

## Core abstraction

```handl
-- lib-route (new, pure)
compile(pattern)        ~> ast                  -- tokenize+parse, once per route
match(ast, tokens)      ~> params | null        -- segment-wise bind
score(pattern)          ~> [Int]                -- specificity vector
dispatch(entries, key)  ~> { entry, params } | null
pipeline(steps)         ~> (ctx) ~> { resp, state }
chain(steps, seed?)     ~> (ctx) ~> resp        -- serve/CLI-ready
end(h)                  ~> (ctx) ~> resp        -- adapt (req) ~> resp
```

- **`tokens`** are `String`s supplied by the transport: HTTP splits
  `req.path` on `"/"`; CLI splits the argv tail on `" "` (or takes argv
  directly — argv is already a token list). Pattern syntax stays
  `"deploy/:env"` either way; `/` in patterns remains the segment
  separator regardless of the runtime delimiter, so one grammar serves
  both.
- **`dispatch`** is transport-blind: caller pre-filters eligibility
  (HTTP: method-compatible entries; CLI: flag-compatibility if ever
  needed) and passes the survivors. Ranking is `score_better`
  verbatim — exact beats `:param` beats `*splat`, left-to-right.
- **`pipeline`** is `lib-http-middleware`'s fold moved verbatim. The
  `{ resp, state }` pair convention stays ("response" reads fine for a
  command result; renaming to `out`/`result` is churn for zero semantic
  gain — revisit only if a third transport objects).
- **`not_found` is not exported** — each transport owns its miss
  response (HTTP: 404/405 records; CLI: usage text + exit code).

## What moves vs. what stays

| `lib-route` (new) | `lib-http-route` / `lib-http-middleware` (stay) |
|---|---|
| pattern tokenizer/parser, `match`, `score`, `dispatch` | method eligibility + 405 `Allow`, `?k=v` query constraints, `req.params`/`query_params` merge, 404/405 records |
| `pipeline`, `chain`, `end`, `as_pair`, `state_get`/`state_put` | cors, logger, request_id, session, auth, csrf, body_limit |
| generic list/record helpers (`alen`, `lmap`, `is_*`) | `route()` map builder?, `href`, `resources`, `form`, `query/1`, `json_body` |

**Open question — route-map builder.** `route({ name: leaf })` joining
keys with `"/"` is 90% generic; the HTTP-isms are method-map leaf
detection (`{get:, post:}`) and the `"/"` join. Options: (a) move a
parameterized `entries(map, {sep, leaf_keys})` to lib-route and keep
`route()` as the HTTP wrapper — CLI gets nested subcommand maps for
free; (b) leave it in http-route, let CLI apps build entry lists by
hand. Recommend (a) but as a **phase 2** — phase 1 ships only
match/dispatch/pipeline, which already unblocks CLI routing.

## lib-http-* become adapters (no API break)

- `lib-http-route` v4: `router(routes)` internally compiles patterns via
  `lib-route`, filters by method, calls `dispatch`, merges
  `params`/`query_params` into `req`, emits 404/405. Public exports
  (`route`, `router`, `match_path`, `query`, `href`, `form`,
  `resources`, `resource`, `not_found`, `json_body`) unchanged — its
  1789-line test suite is the regression net.
- `lib-http-middleware`: re-exports or thin-wraps `pipeline`/`chain`/
  `end` from `lib-route`; HTTP middleware bodies untouched.

## CLI usage sketch

```handl
from "route" import { route_map, chain, end }

sub := route_map({
  deploy: { ":env": (ctx) ~> deploy(ctx.params.env) },
  status:  (ctx) ~> status(),
}, { sep: " " })

main := chain([
  json_output_flag,        -- middleware: --json wraps resp
  verbose_log,             -- middleware: --verbose -> stderr
  end(sub),
])
```

## Kit analysis (P-004 validation)

The kit system earns its keep here: `kit.hnl` is pure curation, so a
new generic lib slots into manifests with **zero format changes** —

- `kit-cli-app`: add `"route"` to `libs` — the missing piece its gap
  analysis already flagged ("nothing reads own-process argv" was fixed
  by mri/stdio; "no subcommand router" is this proposal).
- `kit-web-server`: unchanged — `http-route`/`http-middleware` deps
  pull `lib-route` transitively. Kits stay swappable; glue stays out
  (app skeletons are user-repo business, consistent with the no-glue
  decision).

Honest caveat: kit.hnl v1 cannot express *composition* (which flags
imply which libs) — fine for now, but if CLI apps converge on a
canonical stack, a `kit-cli-app` glue lib (not manifest field) is the
sanctioned escape hatch per P-004.

## Non-goals

- No regex/backtracking matcher (v3 bounded-state compiler already
  rejected this).
- No HTTP concepts in lib-route: no methods, no status codes, no
  headers, no query-string syntax (`?k=v` constraint *checking* against
  a parsed record may move; constraint *parsing* stays HTTP-side).
- No CLI-specific sugar in lib-route (no `--flag` parsing — that's
  mri's job; route matches positional tokens only).
- No new middleware in this proposal.

## Tests

- lib-route: port the v3 compiler/matcher/scoring tests verbatim from
  lib-http-route (they're already transport-free); add dispatch
  tie-break/order tests and pipeline short-circuit/state-merge tests
  ported from lib-http-middleware.
- lib-http-route/http-middleware: existing suites must pass unmodified
  — that *is* the compat guarantee.
- CLI proof: one integration test — argv tail → route_map → handler
  arrow with `:param` binding + one middleware wrapping output.

## Migration plan

1. `lib-route` repo: move pure sections, add pkg.hnl/bupha.toml/
   workflows/AGENTS.md per lib-* template.
2. `lib-http-route`: swap internals to delegate; keep exports; run
   `hnl-gate.sh`.
3. `lib-http-middleware`: delegate pipeline core; keep exports.
4. `kit-cli-app/kit.hnl`: add `"route"` to libs.
