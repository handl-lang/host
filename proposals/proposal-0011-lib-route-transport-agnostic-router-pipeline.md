+++
id = "P-011"
title = "lib-route: transport-agnostic router + pipeline"
status = "open"
created = "2026-11-23"
priority = "med"
labels = ["cli", "http", "routing"]
+++

# P-011 — lib-route: transport-agnostic router + pipeline

***Status:** draft · **New repos:** `lib-route`, `lib-cli-router`,
`lib-msg-router`, `lib-http-openapi`, `tool-codegen-openapi` ·
**Rename:**
`lib-http-route` → `lib-http-router` (clean break, legacy dropped) ·
**Task:** T-138

## Idea

Routing = dispatch over a key; middleware = onion over a ctx record.
Neither is HTTP. Both already exist inside the http libs, more generic
than their names:

- `http-middleware`'s `pipeline`/`chain`/`end` have **zero HTTP imports**
  — `ctx` is a record, `next` an arrow.
- `lib-http-route`'s (renamed `lib-http-router` — see Clean break)
  v3 compiler + specificity scoring work on **segment lists**; HTTP is
  a thin shell (split on `/`, method filter, 404/405).

CLI needs it: `kit-cli-app` has no subcommand router — `deploy/:env`
over argv is the same problem as `/deploy/:env` over a path.

## API

```handl
from "route" import { route_map, chain, end }

sub := route_map({
  deploy: { ":env": (ctx) ~> deploy(ctx.params.env) },
  status:  (ctx) ~> status(),
}, { sep: " " })

main := chain([json_flag, verbose_log, end(sub)])
```

- `compile(routes) ~> trie` / `match(trie, tokens, accept?)` —
  transport supplies tokens (HTTP: `str.split(path,"/")`; CLI: argv;
  msg: `str.split(type, sep)`). Pattern syntax stays `seg/seg`.
- `pipeline(steps)` / `chain(steps)` / `end(h)` — moved verbatim;
  `{ out, state }` pair convention — renamed from `{ resp, state }`
  (transport-neutral; `out` matches the while-loop `{out: …}` idiom).
- `route_map(map, opts)` (phase 2) returns a dispatcher
  `(key) ~> { entry, params } | miss`; transports adapt it into
  `(ctx) ~> { out, state }` terminals — the sketch's `end(sub)`
  assumes that adapter.
- No `not_found` export — each transport owns its miss (404 vs usage).

### Dispatch structure: segment trie, not O(n) scan

Per-request dispatch must not scan every route — `compile` builds a
**segment trie** once at router construction:

- edges: literal segment / `:param` (binds one segment) / `*splat`
  (terminal, binds remainder); `(/optional)` groups compile to
  alternative branches at build time (bounded: ≤8 groups ⇒ bounded
  subtree duplication);
- `match` walks edges in priority order **literal → param → splat**,
  yielding candidates best-first — provably the same winner as
  `score_better` (lexicographic segment score ≡ edge priority;
  strict-prefix-beats-splat ≡ terminal-before-continuation); ties
  keep earliest entry index stored at the terminal;
- `accept(entry)` is the transport eligibility hook (method,
  constraints, `on_miss`): `match` pulls candidates in order until
  one passes — preserves the current "405 only when NO eligible entry
  matches the best path" semantics;
- cost: build O(total pattern size), match O(tokens × branching) —
  vs O(entries × segs) scan; matters once a kit mounts dozens of
  routes (HTTP routers all converged on tries for a reason).

## Companion: lib-cli-router (ergonomics)

`lib-route` stays transport-blind; `lib-cli-router` is the CLI-shaped
convenience wrapper (deps: `route`, `mri`, `stdio`, `posix`).

**Naming:** exports are unprefixed — `router`, `usage`, `run` — the
module name is the namespace, same as `lib-http-router`'s `router()` /
`http-middleware`'s `chain`. Users disambiguate with import aliasing
(`from "cli_router" import { router as cli }`), which handl already
supports (`import { error as js_error }` precedent).

```handl
from "cli_router" import { router, usage }

app := router({
  deploy:  { ":env": deploy_env, ":env/:region": deploy_region },
  status:  status,
  logs:    { "*svc": logs_svc },
}, { about: "deploy platform" })

-- argv in, {code, out} record out; positionals routed, flags -> ctx.state
r := app(argv)          -- { code: 0, out: "..." } | { code: 2, out: usage }
```

What it buys over raw `lib-route`:

- **argv adapter** — drops `argv[0]`, splits positionals vs flags via
  mri, feeds tokens to `route_map` (`sep: " "` baked in).
- **`usage(map)`** — generates `--help` text from the route map for
  free (names + `:params` + `about`).
- **miss → usage + exit 2** — the CLI `not_found`, owned transport-side
  per the lib-route rule.
- **flags as middleware** — `--json`/`--verbose` parsed once, injected
  into `ctx.state`, wrapped by the same `chain`.
- **error boundaries** — `lib-error`'s `error.try(f)` captures
  `error.raise` as a `result`, so a catch-all middleware
  `(ctx,next) ~> error.try(() ~> next(ctx))` works today; no new
  machinery needed.

## Companion: lib-msg-router (message/event dispatch)

Same trick as cli-router, for `{type, ...}` envelopes — ws frames, IPC,
bot commands, any typed-message boundary (deps: `route` only):

```handl
from "msg_router" import { router }

app := router({
  "message/:channel":  on_msg,     -- {type:"message/general", ...}
  "reaction/:emoji":    on_react,
  "presence/*rest":     on_presence,
  "*":                  on_unknown,
}, { sep: "/", type_key: "type" })

res := app(envelope)    -- { req: envelope, params: {...}, state: {} }
```

What it buys over raw `lib-route`:

- **envelope adapter** — operates on *decoded* records only; binary
  payloads (ws frames) are decoded transport-side first
  (`ws-frame-handl`), msg-router never sees bytes. Reads `type_key`
  (default `"type"`, covers `kind`/`event` dialects), splits on `sep`
  (`"/"` or `"."`), feeds `route_map`; the whole envelope stays on
  `ctx.req` so handlers see payload fields alongside `ctx.params`.
- **catch-all `"*"`** — unknown types route to a default handler
  instead of missing; `opts.on_miss` for hard rejects.
- **`doc` → AsyncAPI** — message routes carry the same `doc` record;
  AsyncAPI is the messaging analog of OpenAPI, so `doc` is already
  forward-compatible (a `tool-codegen-asyncapi` sibling later, same
  kernel).
- **middleware reuse** — `chain` wraps it unchanged: per-type logging,
  rate limits, ack/nack wrappers are just `(ctx, next)` arrows.

## Split

| lib-route (new, pure) | stays in lib-http-router / lib-http-middleware |
|---|---|
| compiler, match, score, dispatch | method+405, `?k=v` constraints, req merge, 404/405 |
| pipeline, chain, end | cors, logger, session, auth, csrf, body_limit |
| list/record helpers | `route()` map, `href`, `resources`, `form`, `query` |
| — | `lib-cli-router`: argv adapter, usage gen, exit codes |
| — | `lib-msg-router`: envelope adapter, catch-all, doc→AsyncAPI |

Route-map builder (`route({name: leaf})`) moves phase 2, parameterized
by `sep` + leaf keys — phase 1 ships match/dispatch/pipeline only.
The subtlest part: leaf discrimination. HTTP detects method-maps by
reserved keys (`get`/`post`/…); the generic builder needs a
`leaf_keys`/`spec_keys` config so `doc`/`handler`-style leaves and
plain arrows both work without HTTP assumptions.

**`doc` passthrough:** compiled `entry` records carry `doc` verbatim —
`lib-http-openapi` consumes entries (not the raw map), so doc must
survive `route_map`/`route()` compilation untouched.

## Clean break (approved)

`lib-http-route` is **renamed `lib-http-router`** and rebuilt on
`lib-route` with a clean v2 surface — no legacy API preservation
(pre-1.0, org-internal). `lib-http-middleware` likewise drops the
inlined pipeline core for `lib-route` imports; its HTTP middleware
bodies are untouched. Tests port with the code; behavior parity is
the bar, not signature parity.

## Kit

`kit-cli-app/kit.hnl`: add `"route"` + `"cli-router"` to `libs`.
`kit-web-server`: `"http-route"` → `"http-router"` in `libs`
(module rename lands with the repo rename); `"msg-router"` joins it
for the ws/IPC surface. No manifest format change — P-004 validated.

## Route annotations (`doc`)

Every leaf spec gains an optional `doc` record — transport-neutral
metadata the router ignores but generators consume:

```handl
route({
  photos: {
    index: { get: list_photos,
             doc: { summary: "List photos", tags: ["photos"] } },
    show:  { get: get_photo,
             doc: { summary: "Get a photo", operationId: "photos.show",
                    params: { id: { desc: "photo id" } },
                    responses: { "200": { schema: "Photo" } } } },
  },
})
```

- Lives on spec leaves AND method-map leaves (`doc` joins the allowed
  config keys alongside `path`/`pattern`/`method`/`methods`).
- Known fields map to OpenAPI (`summary`, `description`, `tags`,
  `operationId`, `deprecated`, `params`, `requestBody`, `responses`,
  `security`); unknown fields pass through — forward-compatible.
- `entry.name` is the default `operationId`; `href()`-style names make
  generated clients possible later.
- **CLI bonus:** `usage()` renders `doc.summary` per subcommand —
  same annotation feeds `--help` and OpenAPI.

### OpenAPI compatibility — gaps bridged at generation time

OpenAPI path templating is weaker than the v3 grammar, but every
divergence has a mechanical translation — the generator absorbs it,
the router stays honest:

- `:param` ↔ `{param}` — direct.
- **`*splat`** → emit `{name}` + vendor extension
  `x-handl-pattern: "/files/*rest"` on the path item. Spec stays valid
  OpenAPI; the extension preserves full fidelity for handl-aware
  consumers. (OpenAPI params are single-segment by convention — the
  extension is the honest escape hatch rather than a silent lie.)
- **`(/optional)` groups** → enumerate expansions: each group is
  independently omittable, so `(/a(/b))` yields `/x`, `/x/a`,
  `/x/a/b` as separate path items sharing the operation (nested groups
  only expand when their parent is present — not a free powerset).
  Bounded by OPT_MAX_GROUPS=8 / depth 4; beyond a sane expansion cap
  (say 16 paths per route) fall back to `x-handl-pattern`.
- **`?k=v` search constraints** → `parameters: [{name: k, in: query,
  required: true}]`; `k=v` additionally gets `enum: [v]` (or `const`)
  since the constraint pins the value. Bare `k` → required, no enum.
- **`"*"` method entries** → no OpenAPI wildcard; emit `x-handl-methods`
  or expand to the declared method-map keys when mixed.
- **`operationId`** uniqueness across enumerated optional paths →
  suffix (`photos.show`, `photos.show~a`) or rely on path+method
  uniqueness (operationId is optional in the spec).

Request/response **schemas** stay out of scope — `doc` carries schema
*refs*; a JSON-Schema source is a separate later lib.

### OpenAPI tooling — codegen kernel, not ad-hoc emission

OpenAPI is HTTP-only by definition (the messaging analog is AsyncAPI —
a future `doc` consumer for the message-router use case, same
annotation field). And spec generation is a **build artifact**, not a
runtime call — so it goes through the `tool-codegen` kernel
(Source → IR → Emitter → Plan/Apply over ArtifactStore), which gives
fingerprinted caching and `hnl codegen` integration for free:

- **`lib-http-openapi`** (pure `.hnl`): the transform core —
  `openapi(map) ~> Value` walking route entries + `doc` → OpenAPI doc
  model (the gap-bridging rules above live here). Testable without
  Rust, reusable by any host.
- **`tool-codegen-openapi`** (plugin, `hnl codegen openapi`):
  Source = a handl module exporting the route map (host evaluates it),
  IR = the OpenAPI document model, Emitter = JSON (+YAML later).
  Fingerprinted caching means `openapi.json` regenerates only when
  routes/`doc` change — static artifact, committed or served.
  Named `tool-*` per the post-migration layer rename (engine→tool,
  project P-004); lands today as `tool-codegen-openapi` alongside
  the family and renames with it.
- **openapi.json → handl — `client` + `server` targets** (both needed,
  not optional): `client` emits handl client bindings over `lib-fetch`
  (paths/params from the spec → typed call arrows); `server` emits
  `.hnl` route-map skeletons (paths → `route({…})` leaves with
  `doc:` round-tripped). Same plugin, second Source direction —
  spec file in, `.hnl` packages out.

## Types & effects

Same discipline as `lib-stdio`/`lib-fetch` port contracts — signature +
doc, effect rows mark kernel reach:

```handl
-- lib-route: pure, plain ~>
export compile  : Array(Entry) ~> Trie
export match    : Trie ~> Array(String) ~> (Entry ~> Bool) ~> Match | null
export pipeline : Array(Middleware) ~> Terminal ~> (Ctx ~> Pair)
export chain    : Array(Step) ~> (Req ~> Resp)

-- lib-cli-router: pure core, effectful edge
export router      : RouteMap ~> Opts ~> (Array(String) ~> CliResp)
export run         : (Array(String) ~> CliResp) ~> Array(String) ~[1]~> Unit
                    -- prints out, posix.exit(code)

-- lib-msg-router: pure, envelope in / result out
export router      : RouteMap ~> Opts ~> (Envelope ~> Resp)
```

`router(...)` itself is pure — argv in, `{code, out}` record out;
`run` is the `~[1]~>` shell that does stdio+exit. Tests exercise the
pure arrow; only `run` needs a process.

**Open edge — argv source:** mri parses argv *given to it*; nothing
reads own-process argv today (P-004 flagged `/proc/self/cmdline` or
runner-injected `{"args":…}` as the missing piece). `run` needs one:
either a `posix.argv()` op (`read /proc/self/cmdline`, NUL-split) or
a host protocol field. Small, but it blocks `cli-router` v1.

## DDD fit

The split maps cleanly onto layers:

- **Domain** — handler arrows are pure `(ctx) ~> value`; they see only
  `ctx.params`/`ctx.state`, never argv or HTTP records. A deploy
  handler doesn't know it's behind a CLI.
- **Application** — `route_map` is the wiring/registry; `chain`
  middleware are cross-cutting application services (auth, logging,
  flag injection).
- **Infrastructure** — `lib-http-router`, `lib-cli-router`, `lib-serve`
  are adapters translating transport requests into `ctx` and results
  back. The `{out, state}` pair is the anti-corruption boundary.
- **Bounded contexts** — each context owns a route-map subtree and a
  `state.<ctx>.*` key namespace; right-wins merge lets contexts compose
  without collisions.

## Non-goals

No regex/backtracking, no HTTP concepts (methods/status/headers) in
lib-route, no flag parsing (that's mri), no new middleware.

## Tests

- lib-route: port v3 compiler/matcher/scoring + pipeline tests
  verbatim; dispatch tie-breaks; state-merge semantics.
- lib-cli-router: argv→tokens, usage gen, miss→exit-2, flag injection.
- lib-msg-router: type_key/sep variants, `:param` binding from type
  segments, catch-all `*`, `on_miss`.
- lib-http-openapi: route map + `doc` → OpenAPI doc — param/splat/
  optional/constraint/method bridging rules each covered.
- tool-codegen-openapi: fingerprint caching (unchanged routes → no
  rewrite), `client`/`server` emit round-trip on a fixture spec.
- http libs: ported tests assert behavior parity (clean-break surface).
