+++
id = "P-013"
title = "Atoms as arrows: atom-spec IDL + spec-driven runtime + scoped deps"
status = "open"
created = "2026-11-24"
priority = "high"
labels = ["ffi", "stdlib", "codegen", "typecheck", "architecture", "packages"]
+++

# P-013 — Atoms as arrows: atom-spec IDL + spec-driven runtime + scoped deps

***Status:** draft · **Tasks:** T-148..T-152 · **Supersedes:** ADR-0077
decl forms (`cap`/`op`/`handle`), advances ADR-0164 · **Related:**
ADR-0151 (signatures are typecheck metadata), ADR-0155/0156 (env
records of arrows), ADR-0205 (graded `op`/`handle` keyword collision),
tsconfig `lib`/`types` semantics for package deps

## Why

Every host atom is currently declared **four times by hand**:

1. `cap "mod.fn"` — unenforced plugin metadata (~1.4k lines)
2. `op "mod.fn" : Sig` + `guarantee:`/`requires:`/`termination:` —
   name is load-bearing for file-imported ports; signature and clauses
   are parsed by `plugin-ext-refinement/arrow_parser.rs` and read by
   **nothing** (~2.2k lines)
3. `handle "mod.fn" with rust_impl` — **fully dead**: the Rust name is
   symbolic, never resolved (~1.3k lines); survives only by
   accidentally parsing as the effect-handler `handle`
4. `ModuleSpec { funcs, ops, registry }` + `ops/*.rs` `register(...)` —
   the actual binding authority

Analysis (session 2026-11-24): three strip implementations
(`plugin-lib-handl` bootstrap, `lib-loader`, `handl-runtime`
`module_prep`) already drift; `op`/`handle` keywords collide with the
graded-calculus forms (ADR-0205); `handl "…"` decl form is dead code.

An operation **is** an arrow. The `op "name" : Sig` lines are already
an IDL — the missing piece is that nothing consumes them as data, and
signatures never reach typecheck.

## Design

### 1. `atoms.yaml` — the IDL (one artifact per lib)

```yaml
# lib-utils/atoms.yaml
module: math
atoms:
  - { name: math.abs, sig: "Float ~> Float", impl: math_abs, total: true }
  - { name: math.e,   sig: "Unit ~> Float",   impl: math_e,   total: true }
  - { name: math.add, sig: "(Float * Float) ~> Float", impl: math_add, total: true }
```

- `name` — the qualified atom name (unchanged: `mod.fn` dispatch key)
- `sig` — handl signature, opaque text v1 (parsed by the signature
  reader `arrow_parser.rs` already has; `~>` canonical post-migration)
- `impl` — key into the hand-written Rust impl table (the ONLY
  hand-written part that can't be derived: it IS the computation)
- `total`/future fields — replace the dead `guarantee:`/`requires:`/
  `termination:` clauses; capability gating derives from spec fields
  if ever needed (no `cap` resurrection)

Format: yaml, matching `data/*.yaml` kit conventions — IDLs aren't
terms. (`.hnl`-data alternative rejected: keep IDL out of the term
language.)

### 2. Spec-driven runtime — NO core codegen

The spec is **data**; the runtime builds everything from it:

```
atoms.yaml + impl table (tiny Rust, keyed by `impl:`)
  → OpRegistry entries { name → { fn, sig } }        (T-148)
  → env records of arrow values { fn, sig }          (spec_arrow_record path)
  → typecheck reads sigs off arrow values            (T-148)
```

Per atom, hand-written surface shrinks to: **one yaml line + one Rust
impl fn**. Registry wiring, signatures, `.hnl` exports, typecheck —
all derived. This replaces `ModuleSpec.ops` name lists, `*_op_registry()`
hand-wiring, `MATH_OPS` consts, and the decl triples.

Impl tables stay hand-written but minimal: `ops/math.rs` keeps the
`Abs`/`Add`/`E` `Function` impls; the `register(...)` calls collapse
into a spec-keyed lookup (`impl: math_abs` → `Arc::new(Abs)`), or
self-registration via `inventory`/`linkme` if we want zero wiring.

**File-based native loader (revised T-149):** impls self-register at
link time — `#[handl_atom("math_abs")]` / `inventory::submit!` per
impl fn; a loader crate collects submissions keyed by `impl:` and
`registry_from_atoms(spec)` builds `{name → {fn,sig}}` with **zero
handwritten wiring**. Scope: the ~30 libs vendored into
`plugin-lib-stdlib` (compiled into each ext `.so`); path-dep `.hnl`
libs have no native atoms — consistent boundary.

### 3. Foreign-arrow value shape `{ fn, sig }`

`wrap_dep_fn` already mints callable arrow values (`callv` →
`call_host`). Extend the arrow/`TaggedValue` record to carry `sig`:

- **eval**: unchanged — calls dispatch to the host fn
- **typecheck**: reads `sig` → ordinary arrow types. Atoms typecheck
  *as arrows*, not as special `call_host` leaves — the T-147 bug class
  (`Unit ->` zero-arg calls) cannot recur
- `sig` lives on the value (self-describing, serializable) — atoms are
  few; side-table-by-identity rejected as less uniform

### 4. Scoped deps — tsconfig-`lib` semantics in `pkg.hnl`

Each package declares the atom/module surface it consumes; the entry
file's kit resolves exactly that closure:

```hnl
-- app pkg.hnl
{ deps: { "lib-utils": { atoms: ["math.abs", "math.e"] } } }
-- or coarser: deps: { "lib-utils": "math" }   (whole module)
```

- Ambient surface = **declared deps only** — no global `base_modules`
  leakage; typecheck scope matches eval resolution exactly
- `from "math" import abs` is typesafe: the sig rides the arrow value
- Extends existing `pkg.hnl deps:` (cli-router already uses it) —
  atom granularity is the new bit; whole-module stays the default
- Kit manifests (`kit.hnl libs:`) keep working: they select packages;
  `pkg.hnl` deps select atoms *within* the resolved closure

### 5. `rust` ffi module — dynamic invocation

Uniform host-fn boundary for cases where atoms aren't statically
known (plugins, wasm hosts, generated clients):

```hnl
f := ffi.rust.resolve("math.abs")   -- runtime lookup → arrow value
x := f(-3)
```

Mirrors the C `ffi` module; same `{fn,sig}` arrow shape.

### 6. Codegen — external clients ONLY

`tool-codegen-ffi` consumes `atoms.yaml` → TypeScript/Python/OpenAPI-
style client bindings (sibling of the webidl/openapi backends). The
**core path needs no codegen** — the spec drives the runtime directly.
This is the key revision vs the earlier draft: IDL→both-sides applies
to *external* consumers; internally the spec is interpreted, not
generated.

### 7. Decl-form extinction

- `handle "…"` lines: deleted outright (dead; un-collides graded
  `handle`)
- `cap "…"`: deleted; derive capability lists from spec if needed
- `op "name" : Sig`: transitional — stays ONLY as the file-import
  string-callee until generated/spec-driven exports cover port files
- `handl "…"` parser support: dead code, removed
- Four strip sites edited in lockstep (loader-parity tested)
- **Contract-repo dissolution audit:** decl-heavy repos like
  `lib-utils` (~330 of 350 math.hnl lines are decls) likely dissolve —
  their real content is the Rust ops already in `plugin-lib-stdlib`,
  leaving per-module `atoms.yaml` + thin `.hnl` wrappers. Split into
  per-module spec repos (`lib-math`/`lib-str`/…) or fold specs into
  host `data/` — decide per repo at T-151; repos with real `.hnl`
  logic stay.

## Plan

1. **T-148** — `{fn, sig}` on `OpRegistry`/`Function` + foreign-arrow
   `Value` shape; typecheck reads atom sigs from arrow values.
   No behavior change for existing modules.
2. **T-149** — `atoms.yaml` spec format + spec-driven registry/env
   builder (`registry_from_atoms(spec, impl_table)`); pilot
   `lib-proc` (3 atoms), then `lib-utils`/`math` (67).
3. **T-150** — `pkg.hnl` atom-scoped deps: kit resolves declared atom
   closure; ambient surface = deps only; typesafe `from` imports.
4. **T-151** — decl extinction per-lib: migrate triples → spec;
   delete `handle`/`cap`/`handl` parser support (4 strip sites in
   lockstep); keep `op "name"` string-callee until port imports
   covered.
5. **T-152** — `rust` ffi module + `tool-codegen-ffi` external-client
   emitter + ADR-0077 supersession note.

## Open

- `sig` representation: parsed type-value at registry build vs opaque
  text parsed lazily — lean parsed-once-at-load (sigs are few).
- Atom granularity in `pkg.hnl deps:` — per-atom lists get verbose
  for math (67); maybe `atoms: "math.*"` glob or module-default +
  exclusions. Decide at T-150 with real pkg.hnl examples.
- Self-registration (`inventory`/`linkme`) vs explicit impl table —
  lean explicit table v1 (simpler, no new deps); revisit if impl
  wiring becomes the new mirror.
- Graded atoms (`~[N]~>` sigs): spec `sig` is opaque text v1 — must
  not preclude them (same signature reader as `arrow_parser.rs`).
