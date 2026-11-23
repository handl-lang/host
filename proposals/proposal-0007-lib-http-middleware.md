# P-007 — lib-http-middleware: request-scoped pipeline

***Status:** approved · **New repo:** `lib-http-middleware` · **Task:** T-120
**Depends on:** `lib-http-route` v2+ request/response records (`lib-http-rfc` shapes)

## Motivation

Cross-cutting HTTP concerns — auth, sessions, CSRF, CORS, logging, timing —
are the same handful of wrappers every server rewrites. Remix/Connect/
Express all converged on middleware pipelines; handl should too. Kept OUT
of `lib-http-route`: routing matches URLs, middleware transforms
requests — different axis, different repo.

## Core abstraction

```handl
middleware := Request ~[1]~> Request     -- or {req, next} ~> Response
```

Two styles, pick one at impl time:

- ** onion/next-style**: `(ctx, next) ~> Response` — handler at the end
  of the chain is the router; each middleware can short-circuit
- ** request-transform**: `Request ~> Request` composed left-to-right —
  simpler, can't short-circuit except by erroring

Recommendation: onion-style — auth/CORS genuinely need short-circuit
(401 before routing). `next` is a first-class arrow so middleware nests
naturally.

## Request context

Middleware needs per-request scratch space (session, principal, request
id). Handl has no mutable cells — context is a **record threaded
explicitly**: `ctx` record `{req, state:{}, ...}` passed to `next`;
middleware returns `{resp, state}` or mutates its own copy. State merges
right-wins at each layer (same rule as lib-log child attrs).

## v1 middleware set (each its own arrow, composable via `chain`)

| mw | Does |
|---|---|
| `cors(opts)` | Access-Control-* headers, preflight short-circuit |
| `logger(log)` | method/path/status/duration → lib-log (T-131!) |
| `request_id()` | mint/propagate `x-request-id` into ctx |
| `session(store)` | cookie ↔ session record; needs a store port (lib-kv?) |
| `auth(verify)` | credential → principal in ctx; 401 short-circuit |
| `csrf(check)` | token validation on mutating methods |
| `body_limit(n)` | reject oversized bodies early |

`chain([mw1, mw2, router])` folds right-to-left into one handler.

## Non-goals v1

- Static file serving, compression, rate-limiting (later libs or
  lib-http-static / lib-http-ratelimit)
- WebSocket upgrade handling
- Async streaming bodies (request/response records are already buffered)

## Tests

- Each middleware in isolation (unit) + chain ordering/short-circuit
- Session round-trip with stub store; CSRF token lifecycle; CORS
  preflight; auth 401 path; ctx state merge semantics
- Integration: `serve(chain([cors, logger, auth, router]))` shape test
