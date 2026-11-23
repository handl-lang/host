# P-005 — lib-http-route v3: Remix-style pattern compiler

***Status:** approved · **Repo:** `lib-http-route` · **Task:** T-118
**Depends on:** v2 route maps (landed, 89 tests) · sequenced after compile-js refactor

## Motivation

v2 gives named route maps, `href()`, 405s, specificity ranking, `*name`
captures. Remix 3 goes further with a real pattern grammar. Handl route
declarations are data — richer patterns stay runtime-agnostic and cost
nothing until matched.

## Grammar additions

| Pattern | Meaning | Example |
|---|---|---|
| `(/segment)` | optional group | `/files(/:name)` matches `/files` and `/files/x` |
| `:file.:ext` | suffix variable | `/dl/:file.:ext` → `{file:"a", ext:"zip"}` |
| `?foo=bar` | search constraint | route only matches when query has `foo=bar` |
| `*name` | named splat (v2 has it) | `/static/*path` |

Optional groups may nest (`(/a(/b))`) up to a bounded depth.

## Design: bounded-state matcher, no regex

Remix compiles patterns to a deterministic automaton — no backtracking,
linear in path length. Handl port:

1. **Compile** pattern → segment list `{literal|param|splat|optional-group}`
   + optional search constraints. Compile once per route at `router()`
   build time; cache compiled form on the route record.
2. **Match** = single left-to-right pass over path segments with a small
   state set for optional branches (each optional group = yes/no fork,
   explored in specificity order). Depth cap on nesting (e.g. 4) keeps
   the fork count bounded — worst case 2^depth, depth≤4 → ≤16 branches,
   still trivial vs regex backtracking.
3. **Specificity** extends v2 ranking: literal > param-with-suffix >
   param > optional-present > splat. Optional routes rank below their
   mandatory counterparts so `/files` prefers the non-optional match.
4. **`href()`** must encode the same grammar: optional groups omitted
   when params absent; suffix vars joined with `.`; search constraints
   appended as query. Round-trip property: `match(href(r,p),p) == r`.

## API (additive, v2-compatible)

```handl
route({
  file:   get("/dl/:file.:ext"),
  ver:    get("(/v:version)/status"),
  search: get("/items", {q: "?foo=bar"}),
})
```

Search constraints live on the route record (`{pattern, method, handler,
constraints}`) — `route()`/`match_path` unchanged for v1/v2 inputs.

## Non-goals

- Regex in patterns (explicitly rejected — bounded-state only)
- Full-URL patterns (protocol/host/port) — T-121, deferred
- Type-level param checking (future: typed route-map contracts)

## Tests

- Grammar: every production parses; malformed patterns error at compile
- Matcher: optional nesting, suffix vars, splat+optional combos,
  specificity ordering, 404-vs-405 preserved
- `href` round-trips for all pattern kinds
- Differential: compile N random patterns, assert match results against
  a naive reference matcher
