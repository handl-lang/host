+++
id = "P-014"
title = "Canonical error handling: Result-as-value everywhere, raise reserved for defects, modes deferred"
status = "open"
created = "2026-11-28"
priority = "med"
labels = ["error-handling", "modes", "result"]
links = ["P-013"]
+++

# P-014 — Canonical error handling: Result-as-value everywhere, raise reserved for defects, modes deferred

***Status:** draft · **Type:** cross-repo policy + lib migration ·
**Related:** ADR-0092 (assert/error primitives), ADR-0152 §2
(`__ctor__` result encoding), ADR-0153/0158/0203/0207 (effects &
modes), pl-0002/pl-0004 (MTT modes migration, effects-as-ambient-modes),
P-013 (atoms-as-arrows)

## Why

Handl's error story is currently **three mechanisms at once**, chosen
per-library by accident of authorship:

1. **`Result<T,E>` values** (`lib-result`: `ok`/`err` minting the
   canonical ADR-0152 `__ctor__` record) — the intended error-as-value
   mechanism, but with *no combinators* (no `map`/`and_then`/
   `unwrap_or`), so every consumer hand-rolls `match r { ok(v): …,
   err(e): … }` or local `is_ok`/`is_err` predicates.
2. **`error.raise`** (irreducible native atom; `EvalError` abort) —
   used for *both* genuine contract violations (pop on empty vec,
   malformed route pattern) *and* expected runtime failures
   (`fetch-c` transport errors, `lib-stdio-c` I/O). There is no
   distinction between "programmer defect" and "recoverable failure"
   at the API level.
3. **Sentinel/alternate encodings** — `ffi.load`/`ffi.sym` return
   `{"__tag__": "ok"|"err"}` records (a *different* Result encoding
   that does not dispatch on ctor identity); `lib-serve-epoll` uses
   `null` for "no reply"/"parse failed"; several C libs re-derive
   `try_load → null` fallbacks locally.

The user directive: "ensure we have all programs in handl based on
error-as-value (lib-result). This is our main error handling thing.
But explore the idea of modes for errors too."

## Audit (2026-11-28)

### The canonical mechanism

- `lib-result/src/index.hnl` — `Result<T,E>` former + `ok`/`err`
  constructors minting `{"__ctor__": sym.seeded("data.Result.ok"|"…err"),
  "value": v, "type": {"__former__": …Result, "indices": [null,null]}}`.
  Type + two constructors only — **no combinators**.
- `lib-error` — native atoms: `error.raise(v)` fails evaluation
  loudly (payload → `EvalError::Other`); `error.try(f)` captures a
  raise as a canonical Result ctor value. Only `lib-stdio-c` uses
  `error.try` today.
- FFI codegen (`tool-codegen-c`/sdkgen emitters) already lowers every
  binding to `Result<T, Errno>` — e.g. `lib-curl`'s generated
  `curl_c.hnl` wraps each `ffi.call` as `if r < 0 { err(ffi.errno()) }
  else { ok(r) }`. The *generated* layer is already canonical.

### Who uses what (non-test `.hnl` sources)

| Pattern | Libraries |
|---|---|
| `error.raise` for contract violations | lib-collections (pop/peek/deque/queue/stack/vec), lib-route + lib-http-router (pattern-compile errors, ~23 sites each), lib-posix-c/fs, lib-cli-router, lib-msg-router, lib-datetime |
| `error.raise` for *expected* failures | lib-fetch-c (curl transport errors), lib-tls-libssl, lib-compress-{gzip,zlib}-c, lib-str-c/str-js (ICU errors), lib-loader, lib-log, lib-http-{middleware,openapi,ratelimit,static}, lib-concurrent-{js,epoll}, lib-stdio-c (16 sites — the heaviest mixed user; also the only `error.try` consumer), lib-unicode-c, lib-ws-whatwg-c |
| Result values, canonical `__ctor__` | codegen'd bindings (lib-curl, and by extension every sdkgen/c-codegen lib); `lib-test` assert/property/fuzz/snapshot + `lib-mock` (hand-minted via `sym.seeded`, deliberately — `HandlModule` binds carry only declared imports); `lib-json-handl` (own `Result<e,a>` Inductive + `PErr`, `mk_ok`/`mk_err` via `while`-thunk mint of the same shape) |
| Result values, *other* encoding | `ffi.load`/`ffi.sym` → `{"__tag__": "ok"|"err", "value": …}` (plugin-ffi `lib.rs` ~513–550); consumed via per-lib `is_ok := match r { ok(_): true, _: false }` redefinitions in lib-json-c, lib-str-c, lib-unicode-c, lib-intl-c |
| Sentinel values | `try_load → null` fallbacks (all *-c libs); lib-serve-epoll `null` reply / `parsed == null` / `parsed.req == null` |
| `handle`/`perform` effects | orthogonal — IO/algebra effects, not error handling; no lib uses `perform` for errors today |

### Rust / host boundary

- Every `Op::call` returns `Result<Value, EvalError>`; `Err` is the
  loud channel (≈ `raise`). Ops that want error-as-value must mint
  the Result record themselves — `plugin-ffi` does this for
  `load`/`sym` (with the divergent `__tag__` shape), codegen does it
  for call bindings (canonical `__ctor__` shape).
- `error.raise`/`error.try` are spec-driven native atoms
  (`lib-error`, `atoms.yaml`, `plugin-lib-stdlib` `ops/kernel_impls.rs`).

### Divergence costs already visible

- The `__ctor__` vs `__tag__` split means `match r { ok(v): … }`
  cannot be trusted across the FFI boundary; each *-c lib carries a
  private `is_ok`/`is_err`/`res_tag`/`payload`/`or_raise` toolkit
  (~4 near-identical copies).
- `lib-json-handl` re-implements `Result` as its own Inductive and
  mints `__ctor__` records through a `while { d: null }` thunk trick
  rather than importing `result` — three independent mint sites
  (`lib-result`, `lib-json-handl`, `lib-test`/`lib-mock`) plus the
  Rust-side `error.try`.
- `fetch` — the highest-value error-as-value candidate (HTTP
  transport failure is *expected*, not a defect) — raises instead of
  returning `Result<Response, FetchError>`.

## Literature grounding

- **Rust `Result`/`?`**: error-as-value with `#[must_use]`;
  propagation is explicit-but-ergonomic. Handl's explicit `match` is
  the pre-`?` equivalent; the missing piece is *combinators*
  (`and_then`/`map`/`unwrap_or`), not syntax — a `?`-like operator is
  a possible later sugar, not required for canonical-ness.
- **Haskell `Either`/`ExceptT`, Scala `Try`, Swift `Result`**:
  consensus — expected/domain failures are values; defects
  (exceptions/panics/`fatalError`) stay on the loud channel. Scala
  benchmarks show `Either` ≈ exceptions perf-wise and strictly more
  composable; the community rule "`Either` for logical errors,
  exceptions for system failures" maps directly onto our
  Result-vs-`raise` split.
- **Algebraic effects (Plotkin & Pretnar; Bauer & Pretnar Eff; Koka
  `exn` rows)**: `raise` as `perform "raise"` + handler is viable in
  principle — Handl *already has* `handle`/`perform` — and the
  approved `design/effects-ambient-modes.md` (pl-0004 v2) literally
  sketches `Raise := new Algebra { op : Err ~> a }`. Koka's graded
  `exn<E>` rows show the payoff (checked exception types in the
  signature) and the cost (row-polymorphism solver complexity —
  Koka's own `exn` is famously the hardest effect to get right).
- **Handl's own modes machinery**: pl-0002/pl-0004 implement
  category-graded arrows — `new Algebra`, `@ [Head(payload)]`
  ambient-mode ascription, `>>>` composition computing mode sets,
  `requires` implication, handlers as modalities `μ → μ′`. The
  granular-effects migration (G0–G2) is *in flight on `mtt-modes`*.
  An `Error(E)`/`Raise` algebra is therefore a *data* addition once
  that lands — no engine work — but mode *tracking* of raise
  capability is exactly the "ambient mode contexts" research-grade
  piece `effects-as-modes.md` already flagged as out of scope until
  pl-0004 completes.

## Proposal

### 1. Canonical error story (the policy)

- **`Result<T,E>` is THE mechanism for recoverable, expected
  failures** — parse errors, I/O failures, transport errors,
  lookups-that-can-miss (paired with `lib-option` for pure absence).
- **`error.raise` is reserved for defects and contract violations** —
  `vec.pop` on empty, malformed patterns, impossible states —
  Rust-`panic!`/`unwrap` semantics, and for the *host boundary*
  (`EvalError` from native ops that cannot produce a value).
- **`error.try` remains the one sanctioned bridge** — wrapping
  defect-throwing or legacy code into a `Result` at the boundary.
- Every public lib API that can fail *in normal operation* returns
  `Result`; APIs that raise document "raises on contract violation".

### 2. Grow `lib-result` into the full small API

Add to `lib-result` (pure `.hnl`, same `__ctor__` encoding):

- predicates: `is_ok`, `is_err`
- transforms: `map`, `map_err`, `and_then`, `or_else`
- extraction: `unwrap` (raises on `err` — the `?`-equivalent
  boundary), `unwrap_or`, `unwrap_or_else`, `or_raise`
- keep `ok`/`err` as the only constructors; `Result<T,E>` stays a
  hand-minted former until `new Inductive` + `export` projects ctor
  names (the lib-result header already notes this).

This kills the ~4 duplicated `is_ok`/`is_err`/`or_raise` toolkits in
the *-c libs and gives `lib-json-handl`/`lib-test`/`lib-mock` a real
import target instead of hand-minting.

### 3. One encoding, one mint site

- **Migrate `ffi.load`/`ffi.sym`** (plugin-ffi) from
  `{"__tag__": "ok"|"err"}` to the canonical `__ctor__` Result record
  — or ship a documented `result.from_tag` adapter if the tag shape
  must stay for compat. New code should never see two Result shapes.
- **Consolidate mint sites**: `lib-json-handl` should `from "result"
  import { ok, err }` (or declare the `Result` former once and share);
  `lib-test`/`lib-mock` keep `sym.seeded` only if the dep graph
  forbids the import — in that case add a `lib-result` conformance
  test asserting all mint sites produce byte-identical records.
- Deprecation: `ffi.load`/`ffi.sym` tag shape gets a deprecation note
  in `lib-ffi` docs; no hard break until consumers migrate.

### 4. `raise` migration policy per library

- **Convert to `Result`**: `lib-fetch`/`lib-fetch-c`/`lib-fetch-js`
  (define `FetchError` — status/transport/parse), `lib-serve*`
  (replace `null` sentinels with `Result<Reply, ServeError>` or
  `Option` where absence is genuine), `lib-posix-c/fs` (fs errors are
  expected — `Result`), `lib-concurrent-*` task failures,
  `lib-tls-libssl`, `lib-compress-*`, `lib-str-c` ICU paths,
  `lib-json-c` parse errors (align with `lib-json-handl`'s `PErr`).
- **Keep `error.raise`**: collections contract violations, route
  pattern compile errors, `proc.exit`, loader/internal invariants —
  document each as "raises on misuse".
- **`error.try` stays** as the interop shim; consider a lint
  (`source.*`-style native lint) flagging `error.raise` inside
  functions whose contract says "returns Result".

### 5. Modes for errors — deferred, with a reserved slot

**Do not build error-type tracking now.** Rationale:

- The value-level story (Result everywhere) is achievable today with
  zero engine work and covers the user's "all programs error-as-value"
  goal completely.
- Raise-*tracking* ("this arrow may raise `E`") is precisely the
  ambient-mode-context machinery pl-0004 is building for *effects*;
  `effects-as-modes.md` §5 already ruled the same deferral for
  `IO<sig>`. Duplicating that analysis for `Raise` before pl-0004 G4
  merges would fork the design.
- **Reserved slot**: once pl-0004 lands, declare
  `Raise := new Algebra { op : Err ~> a, requires : [] }` in
  `lib-error` (or a new `lib-effect-error`) and re-express
  `error.try` as `handle … with { raise: … }`. `Result` remains the
  *value* encoding; `Raise` becomes the *ambient-capability*
  annotation — complementary, exactly like Koka's `exn` row vs its
  `Result`-returning APIs.
- If the user wants *checked error types in signatures* (the Koka
  payoff), that is a follow-up proposal gated on pl-0004 G4 + the
  mode-metavariable solver (E3) — not this one.

### 6. Migration sequencing

1. **P1**: extend `lib-result` with combinators + predicates; add
   conformance test for the `__ctor__` shape.
2. **P2**: `ffi.load`/`ffi.sym` encoding decision + consumer sweep
   (lib-json-c, lib-str-c, lib-unicode-c, lib-intl-c → import
   `result` helpers, delete local copies).
3. **P3**: per-lib Result migration in the order above; each lib is
   its own repo/PR-sized change.
4. **P4** (post-pl-0004): `Raise` algebra + `error.try` re-expression;
   optional signature-level raise tracking.

## Open questions for the user

1. **`err` payload discipline**: free `E` (current) vs a canonical
   `Error` sum type (`{kind, message, cause?}`) for interop? Free `E`
   is more Haskell-`Either`; a canonical `Error` is more
   Swift/Rust-ergonomic. Proposal defaults to free `E` + a
   `lib-error`-provided `Error` record constructors lib authors may
   adopt.
2. **`fetch` shape**: `Result<Response, FetchError>` vs
   `Response`-or-raise + documented `error.try(fetch, …)`? Proposal
   defaults to `Result` (fetch failure is the textbook expected
   error).
3. **`unwrap` naming**: `unwrap` that *raises* vs `expect(msg)` —
   keep both? (Rust precedent: `unwrap`/`expect`.)
4. **Should `error.raise` gains a `Result`-returning variant name**
   (`result.raise`?) or stay `error.*`-namespaced? Proposal keeps
   `error.*` (one module, one concept).
5. **Lint enforcement**: worth a `hnl-tool-lint` native rule
   "public arrow annotated/declared Result must not `error.raise`
   expected failures"? Or keep it convention + review?
