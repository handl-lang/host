# ADR-0077: Stdlib Migration to Handl — `op` Declarations, Axioms, and the Elimination of Per-Library Rust

## Status

Accepted. Builds on [ADR-0069 — Arrow-Native Modules](./0069-arrow-native-modules.md),
[ADR-0052 — Scheme IR Contracts](./0052-scheme-ir-contracts.md),
[ADR-0068 — Predicates as Types](./0068-predicates-as-types.md),
and [ADR-0051 — FFI Rust Binding Facade](./0051-ffi-rust-binding-facade.md).

## Context

### The current state

The `plugin-lib-*` family of crates (~60 crates) implements the handl
standard library. Each crate is Rust. They fall into two populations:

1. **Pure-logic libs** (`hello`, `stats`, `collections`, `utils`, `parse`,
   …) — the logic is already handl-authored in `.hnl` files under
   `handl-packages/`, but each crate carries ~400 lines of hand-rolled
   Rust bridge code (`bridge.rs`, `module.rs`, `plugin.rs`, `lib.rs`) that
   `include_str!`s the `.hnl` source, builds a cached per-thread `Runtime`,
   and evaluates `(source).func(args)` per call.

2. **Native-primitive libs** (`base64`, `crypto`, `fs`, `http`, `tls`,
   `os`, `rand`, …) — the logic _and_ the implementation are Rust, because
   they wrap Rust crates or syscalls. There is no handl source for them.

### What already exists

- **Eval-side module registry** (`plugin-module`): `Module` trait,
  `ModuleRegistry`, `ModuleLookup`, `ModuleRule`/`ModuleRuntime`. The
  tree-walk eval engine dispatches `ArrSource::Foreign(name)` to
  `ModuleLookup::lookup(name)` → Rust `Function::call(args)`.
- **Inference-side module loader** (`plugin-ext-refinement`):
  `ModuleLoader` (ADR-0069) loads `.hnl` files, parses `import`/`export`,
  produces `HashMap<String, Scheme>` (named arrow graphs) + cached
  contracts. All three normalization strategies (A: compile-time desugar,
  B: flatten, C: runtime lazy load) are implemented. This feeds
  refinement inference only — **not** eval.
- **Axiom system** (`plugin-ext-refinement`): `axiom name : T -> T,
guarantee: P` declarations in `.hnl` contract files. Loaded into
  `ContractRegistry`. Used for static refinement inference. **Not** wired
  into the eval-time foreign-call boundary.
- **`ArrSource::Foreign(name)`** in `Scheme` IR — the capability-tracked
  foreign-call marker. The tree-walk engine dispatches it to
  `ModuleLookup`. But the foreign name, its type, and its contract all
  live in Rust, not in handl source.

### The gap

The two module systems (eval-side registry, inference-side loader) are
not connected. There is no path from a `.hnl` file to a callable eval-time
`Module` without a per-library Rust bridge. And for native libs, there is
no handl surface syntax for declaring foreign bindings — the foreign
name, type, and contract are all Rust-authored.

A partial bridge (`plugin-lib-handl` with `HandlModule` /
`HandlLibraryPlugin`) was just built and proven by migrating
`plugin-lib-stats`. It eliminates the per-library bridge for pure-logic
libs. But it does not address native libs, axioms, or capability
declaration.

### Why axioms and proofs must live in handl

Rust offers no proof mechanism. The stdlib's correctness story —
refinement contracts, termination guarantees, compositional inference —
can only be expressed and verified in handl. Today the axioms for
irreducible Rust primitives live in `handl-packages/contracts/prim.hnl`,
but they are disconnected from the actual Rust `Function` impls they
describe, and they are not enforced at the eval boundary.

For the migration to be complete, every public arrow in the stdlib must
carry:

- A `---` doc comment (the public API contract in prose).
- An `axiom` declaration (the formal contract: input type, output type,
  `guarantee`, optional `requires`/`termination`).
- A proof (where the body is handl-authored — the proof is compositional
  via the contract cache, ADR-0069 §4).

For native libs, the Rust body cannot be proven. But the _axiom_ can be
stated in handl, and the Rust body is trusted to honor it. This is the
FFI boundary: handl declares and verifies the contract; Rust implements
it.

### Requirement: pure-handl `result` library for error handling

**This is a hard requirement, not a nice-to-have.** The stdlib cannot be
written in handl without a pure-handl `result` type for graceful error
handling. Today the `result` pattern (`result.ok`, `result.err`,
`result.is_ok`, `result.is_err`) is implemented in **Rust** —
`ResultModule` in `plugin-lib-utils/src/result/module.rs` constructs
`{"_tag": "Ok", "value": <T>}` / `{"_tag": "Err", "error": <E>}` JSON
objects via Rust `Function` impls. The `.hnl` file
(`handl-packages/utils/utils.hnl`) carries doc comments and `cap`
declarations for these, but the implementation is Rust.

This blocks the migration: every handl-authored stdlib function that can
fail (parse, lookup, conversion, FFI calls) needs `result` to return
errors structurally instead of crashing. If `result` stays Rust, then
handl stdlib code depends on a Rust module for error handling —
defeating the point of the migration and creating a circular dependency
(handl stdlib -> Rust result module -> handl stdlib for its own logic).

**The `result` library must be migrated to pure handl as part of Phase
1, before any other stdlib function that uses error handling is
migrated.**

### Requirement: language-level tag declarations (prerequisite for `result`)

The pure-handl `result` library requires a **language-level tag system**
-- a nominal identity registry so that `Ok` and `Err` are first-class
tags with structural identity, not hand-built `{"_tag": "Ok", ...}` JSON
objects that rely on string-matching conventions.

**What already exists -- `plugin-lib-tag`:**

The tag system already exists as a host-exposed library
(`crates/plugin-lib-tag/`, ADR-0055). It exposes seven arrows declared
in `handl-packages/tag/tag.hnl` via the `op`/`handl` form:

| Arrow                          | Role                                                    |
| ------------------------------ | ------------------------------------------------------- |
| `tag.tag(name, payload)`       | **mint** a tagged value `{"__tag__": name, ...payload}` |
| `tag.untag(v)`                 | return the payload of a tagged value                    |
| `tag.tag_of(v)`                | return the tag name string, or null                     |
| `tag.is_tag(v)`                | test whether `v` is a tagged value                      |
| `tag.construct(name, payload)` | mint a registry-scoped type value (ADR-0054)            |
| `tag.match_type(v, cases)`     | dispatch on the type tag (ADR-0054)                     |

`tag.match_tag` is **not needed** -- the language already has a `match`
construct that dispatches on tag identity. The tag sugar's
`match r { ok(v) => ... }` desugars to the native `match`, not to a
library arrow. `tag.match_tag` can be removed from `plugin-lib-tag`.

The `__tag__` key is **reserved** -- user object literals cannot produce
it (the eval engines reject it), so the only way a value acquires a
`__tag__` key is through `tag.tag` (forgery prevention, ADR-0055 S3).
This is already a nominal identity system: `tag.tag("ok", {value: 5})`
and `tag.tag("err", {value: 5})` are distinguishable by tag name, not by
string-matching a user field.

**What is missing -- surface syntax sugar:**

The tag library arrows exist, but there is no surface syntax to declare
and use them ergonomically. Today you'd need to manually compose arrows
that construct `tag.tag` payloads -- verbose and error-prone (tag names
are string literals, not declared entities). The `tag` keyword is
**sugar** that generates the constructor arrows at the Scheme level --
no new primitive, no engine change, no surface-syntax `tag.tag(...)`
calls.

**What the sugar looks like:**

Declaring tags:

```handl
tag ok = Value
tag err = Error
```

This desugars at the **Scheme level**, not in surface syntax. The
desugar pass registers `ok` and `err` in the tag registry (a
data-driven `Language` entry, per ADR-0044) and generates constructor
arrows directly as arrow-calculus `Scheme` -- composing `arr` (lift the
payload construction) with `&&&` (fanout: tag name constant + payload)
and `>>>` (flow into the `tag.tag` host arrow). No surface-syntax call
to `tag.tag(...)` -- that would be function application, not arrow
composition.

The declaration also registers the tag name + payload type in the
`Language` tag registry so the typechecker can enforce tag identity
nominally (an `ok` value is not an `err` value, even if the payload
type is the same).

Constructing a tagged value -- compose via `>>>` (arrow flow):

```handl
42 >>> ok        -- ok is an arrow: Value -> Tagged ok Value
"nope" >>> err   -- err is an arrow: Error -> Tagged err Error
```

`ok` and `err` are arrows. You compose them in a pipeline, you don't
apply them like functions. This is arrow calculus, not lambda calculus.
The `tag.tag` host arrow is wired in at the Scheme level during
desugar -- the user never writes `tag.tag(...)` call syntax.

Checking tag identity (sugar for `tag.tag_of` + compare):

```handl
r is ok(_)    -- desugars to: tag.tag_of(r) == "ok"
r is err(_)   -- desugars to: tag.tag_of(r) == "err"
```

Pattern matching uses the language's native `match` -- no library
arrow needed:

```handl
match r {
  ok(v)  => on_ok(v),
  err(e) => on_err(e),
}
```

The native `match` dispatches on tag identity and binds the payload
fields by name, so `ok(v)` binds `v` to the `value` field of the
`ok`-tagged payload. No `tag.match_tag` library arrow required.

**Relationship to ADR-0082 (`data`):**

`data` declarations (ADR-0082) generate predicates, constructors, and
recursors for inductive types. The constructors today use
`{ tag: "Zero" }` string-keyed JSON objects -- unregistered, no nominal
identity, forgeable by user code. With the tag sugar, `data` constructors
would desugar to `tag.tag` calls instead:

```handl
data Nat = Zero | Succ(Nat)
```

Would generate constructors like:

```handl
tag zero = Unit
tag succ = Nat

-- Constructors are arrows, composed via >>>:
-- () >>> zero   -- zero is an arrow: Unit -> Tagged zero Unit
-- n >>> succ    -- succ is an arrow: Nat -> Tagged succ Nat
```

This gives `data` constructors the same nominal identity + forgery
prevention as user-declared tags. The `data` desugar (ADR-0082) would
emit `tag` declarations instead of string-keyed object literals.
However, `data` is a larger feature (recursors, predicates, strict
positivity); the tag sugar is the simpler prerequisite that both
`result` and `data` build on.

**Design notes:**

- The `tag` keyword is **pure sugar** -- it desugars to calls to
  `tag.tag` (construction) and `tag.tag_of` (inspection). Pattern
  matching uses the language's native `match`, not a library arrow. No
  new native op, no engine change, no new `TaggedValue` variant. The
  host capability (`plugin-lib-tag`) already does the work.
- Tags are a **nominal** type system feature: identity is by tag name,
  not by structural shape. `ok(5)` and `err(5)` have the same payload
  but different tag identity. The `__tag__` key is reserved so user
  code can't forge tagged values.
- Tag declarations are **data-driven**: a `tag` declaration adds an
  entry to the `Language` tag registry (tag name + payload type). No
  engine recompile. This is consistent with ADR-0044 ("language as
  data") and the engine-genericity rule.
- Tag declarations should be **exportable** (via the `export` mechanism,
  Phase 6) so that `result`'s `ok`/`err` tags are importable by any
  stdlib module that needs error handling.

**Implementation plan:**

1. Add `tag` declaration syntax to the grammar (`grammar.yaml`) and AST
   (`ast.yaml`): `tag <name> = <payload_type>` as a top-level form.
2. Add a tag registry to `Language` (data-driven): a map from tag name
   to its payload type. Tags are registered when a `.hnl` file declaring
   them is loaded.
3. Desugar `tag <name> = <type>` to a constructor arrow at the Scheme
   level -- compose `arr` (payload construction) with `&&&` (fanout:
   constant tag name + payload) and `>>>` (flow into `tag.tag` host
   arrow). The constructor is an arrow `<name> : Payload -> Tagged`,
   composed via `>>>`, never called via function-application syntax.
   Field names match the payload type's fields if it's a record.
4. Desugar `expr is <tag_name>(<var>)` to `tag.tag_of(expr) == "<tag_name>"`.
5. `match expr { <tag>(<var>) => body, ... }` uses the language's
   native `match` -- no desugar needed, just ensure `match` recognizes
   tag patterns (dispatch on `__tag__` identity, bind payload fields).
6. Remove `tag.match_tag` from `plugin-lib-tag` -- it's redundant with
   the native `match`.
7. Migrate `result` to use `tag ok` / `tag err` sugar instead of
   `{"_tag": "Ok", ...}` JSON objects.
8. Migrate `data` constructors (ADR-0082) to desugar to `tag`
   declarations (generating constructor arrows at the Scheme level)
   instead of `{ tag: "..." }` string-keyed objects.

**Done when:** `tag ok = Value` / `tag err = Error` can be declared in
`.hnl` source; `v >>> ok` / `e >>> err` compose with the constructor
arrows (which wire into `tag.tag` at the Scheme level); `r is ok(_)`
checks via `tag.tag_of`;
`match r { ok(v) => ..., err(e) => ... }` dispatches via the native
`match`; `tag.match_tag` is removed from `plugin-lib-tag`; the
typechecker recognizes tag identity; `result` is implemented using the
tag sugar, not JSON objects. This is a prerequisite for the pure-handl
`result` library, which is itself a prerequisite for Phase 1.

**The pure-handl `result` implementation using tag sugar:**

Only the constructors need to migrate. Predicates (`is_ok`, `is_err`),
extraction (`unwrap_or`), and dispatch (`match`) are already covered by
`mood` (the core interrogative feature) and native `match` -- no
wrappers needed.

`mood` is the core language feature for interrogatives:

- `mood.int(r, ok)` -- is r an ok-tagged value? (replaces `result.is_ok`)
- `mood.int(r, err)` -- is r an err-tagged value? (replaces `result.is_err`)
- `mood.norm(r, ok)` -- project ok payload or return err result
- `mood.ind(r, ok)` -- project ok payload or error
- `match r { ok(v) => ..., err(e) => ... }` -- native language match (replaces `result.match`)

Combinators like `map` and `and_then` are pure handl stdlib functions
built on `mood` + native `match` -- they don't need to be part of this
migration.

```handl
-- Declare the tags (desugars to tag.tag constructor arrows).
-- Exported so importers get the tag identity too.
tag ok = Value
tag err = Error

--- result.ok -- construct an Ok result wrapping a value.
--- Contract: IsOk(output)
result.ok = value ~> value >>> ok

--- result.err -- construct an Err result wrapping an error.
--- Contract: IsErr(output)
result.err = error ~> error >>> err
```

`ok` and `err` are arrows (`Value -> Tagged`). `result.ok` composes
the input through `ok` via `>>>` (arrow flow). No function application.

That's it -- two one-liner arrows. The Rust `ResultModule`
(`plugin-lib-utils/src/result/module.rs`) is deleted; its `ok`/`err`
`Function` impls are replaced by the two arrows above. Its
`is_ok`/`is_err` impls are deleted entirely -- use `mood.int(r, ok)` /
`mood.int(r, err)` directly. The combinator functions, if they exist in
Rust today, are already or will be pure handl using `mood` and
native `match`.

Each of the two arrows composes with the `ok`/`err` constructor arrows
(generated at the Scheme level from the `tag` declarations, which wire
in the `tag.tag` host arrow). No surface-syntax `tag.tag(...)` calls.
No new primitive.

**Done when:** `result.ok`/`err` are pure handl arrows composing with
the `ok`/`err` tag constructor arrows via `>>>`; the Rust
`ResultModule` and its `Function` impls are deleted; `mood.int(r, ok)`
/ `mood.int(r, err)` cover predicate checks; all tests pass.

## Decision

Migrate the entire `plugin-lib-*` stdlib to handl-authored `.hnl` files,
eliminating per-library Rust crates. The migration proceeds in six
phases, in dependency order. Each phase is independently shippable.

### Required content of every migrated `.hnl` file

Every public arrow in the stdlib must carry:

1. **`---` doc comment** — the public API contract in prose, immediately
   above the arrow definition. This is the documentation surface (rendered
   by the doc generator, shown in LSP hover).

2. **`axiom` declaration** — the formal contract: input type, output
   type, `guarantee`, optional `requires`/`termination`. This is the
   verification surface (consumed by the inference engine, enforced at
   the eval boundary after Phase 3).

3. **Proof** — for handl-authored arrows, the proof is compositional via
   the contract cache (ADR-0069 §4): each arrow is inferred once, its
   contract cached, importers reuse it. No separate proof file is needed
   — the inference engine generates the proof from the arrow graph +
   imported contracts. For `op` arrows (Rust-backed), no proof is
   generated — the axiom is trusted (the Rust handl is assumed to honor
   it), but the contract is still enforced at the boundary (Phase 3).

Example of a complete migrated pure-logic arrow:

```handl
--- stats.sum — sum all numeric elements in an array.
--- Non-numeric elements are silently skipped.
--- Contract: SumOf(output, input)
sum = xs ~>
  loop { xs: xs, acc: 0 }(
    [xs, acc] ~> match xs {
      []: { out: acc },
      [h, ...t]: if type.type_of(h) == "num" then
        { state: { xs: t, acc: acc + h } }
      else
        { state: { xs: t, acc: acc } }
    }
  )
```

Example of a complete migrated native `op`:

```handl
--- base64.encode — encode a string to base64.
--- Contract: IsBase64(output)
op "base64.encode" : String -> String
  guarantee: IsBase64(output)

handl "base64.encode" = rust_base64_encode
```

### Phase 1 — Pure-logic libs via `HandlModule` (in progress)

Migrate all pure-logic libs (`hello`, `collections`, `utils`, `parse`,
…) to `.hnl` files loaded by the generic `plugin-lib-handl` crate
(`HandlModule` / `HandlLibraryPlugin`), following the proven
`plugin-lib-stats` migration.

**Prerequisites:** The language-level tag system (see "Requirement:
language-level tag declarations" above) and the pure-handl `result`
library built on it (see "Requirement: pure-handl `result` library for
error handling" above) must land first. Every other stdlib function
that can fail needs `result` for error handling — migrating them before
`result` is pure handl would leave them dependent on the Rust
`ResultModule`, creating a circular dependency. And `result` itself
needs the tag system so that `Ok`/`Err` are nominal tags, not
string-keyed JSON objects.

Each `.hnl` file carries:

- `---` doc comments on every public arrow (the API contract in prose).
- The arrow logic (already handl-authored for hello/stats; port the rest
  from Rust).

The Rust crates collapse to: one `plugin-lib-handl` crate (already built)

- N `.hnl` files + a kit manifest entry per lib. No per-library Rust.

**Done when:** `plugin-lib-hello`, `plugin-lib-collections`,
`plugin-lib-utils`, `plugin-lib-parse` (and any other pure-logic libs)
are migrated; their Rust bridge code is deleted; all tests pass.

### Phase 2 — `op` declaration form (FFI)

Add surface syntax for declaring operations (Rust-backed functions) in
`.hnl` source, so native libs can declare their interface in handl
without a Rust `Module` struct. The declaration is an **operation** in
the algebraic-effects sense (Plotkin & Pretnar; Sanada's arrow calculus
with operations and handlers, ADR-0052 §"Arrow Handlers"); the Rust body
that implements it is the **handl** (the handler that interprets the
operation).

```handl
-- fs.hnl
op "fs.read" : Path -> Bytes
  requires: PathValid(path)
  guarantee: IsUtf8(output)

op "fs.write" : (Path × Bytes) -> Unit
  guarantee: True
```

The `handl` block binds a Rust function (the handler) to an `op`:

```handl
handl "fs.read" = rust_fs_read
handl "fs.write" = rust_fs_write
```

This requires:

- **Lexer/Pratt parser support** for `op` and `handl` declarations (new
  top-level forms, like `import`/`export` in ADR-0069). Keywords
  `token.keyword.op` and `token.keyword.handl` are already registered in
  `tokens.yaml` and `lexer_rules.yaml`.
- **An `OpDecl` node** in the parsed module, carrying the name, type
  signature, and optional axiom (requires/guarantee/termination).
- **A `HandlDecl` node** in the parsed module, carrying the op name and
  the Rust function identifier to bind.
- **A host-side registry** mapping declared `op` names → Rust `Function`
  impls (the handls). The Rust bodies stay; only the declaration moves
  to handl.
- **`HandlModule` integration** — when a `.hnl` file contains `op`
  declarations, the loaded module exposes them as callable functions that
  dispatch to the Rust registry (same `ArrSource::Foreign` path, but the
  name/type/contract now come from handl source, not Rust).

**Done when:** at least one native lib (e.g. `plugin-lib-base64`) is
migrated — its `Module`/`Function` trait impls deleted, replaced by an
`op` declaration in `.hnl` + a Rust `Function` (the handl) registered
under the declared name. All tests pass.

### Phase 3 — Wire axioms to eval

Attach `ContractRegistry` contracts to the `Foreign` dispatch boundary so
that declared `op`s carry enforced `guarantee`/`requires`, not just
inferred offline.

This requires:

- **Linking `OpDecl` axioms to `ContractRegistry`** — when a module
  with `op` declarations is loaded, its axioms are registered in the
  contract registry under the op name.
- **Eval-time checking** — the tree-walk engine, when dispatching
  `ArrSource::Foreign(name)`, consults the contract registry for `name`
  and checks the `requires` predicate against the call arguments (at
  runtime; static checking is the inference engine's job).
- **Proof obligation** — for handl-authored arrows, the proof is
  compositional (ADR-0069 §4 contract cache). For `op` arrows, the
  axiom is trusted (the Rust body is assumed to honor it); no proof is
  generated, but the contract is still enforced at the boundary.

**Done when:** an `op` with a `requires` predicate rejects a call
that violates it; an `op` with a `guarantee` is checked against the
return value (where feasible). Tests cover both.

### Phase 4 — Native libs migrated to handl interfaces

**This phase is replaced by [ADR-0083 — FFI-Native Stdlib](./0083-ffi-native-stdlib.md).**

This phase proposed keeping Rust `Function` bodies as the `handl`
implementations for native libs (`handl "fs.read" = rust_fs_read`). ADR-0083
replaces this with direct C/POSIX FFI data bindings (`handl "libc.open" =
host.ffi_bind "libc" "open" ...`), eliminating the Rust body layer entirely.

The `op`/`handl` declaration form (Phase 2) and axiom wiring (Phase 3)
remain valid. Only the `handl` binding target changes: `host.ffi_bind`
instead of `rust_x`. See ADR-0083 for the full design and migration plan.

**Historical text (superseded):**

Migrate all native-primitive libs (`base64`, `crypto`, `fs`, `http`,
`tls`, `os`, `rand`, …) to `.hnl` interface files using the Phase 2
`op` form + Phase 3 axiom wiring.

Each native lib was to become:

- A `.hnl` file with `op` declarations + `---` doc comments + axioms.
- A Rust crate that registers `Function` impls (the handls) under the
  declared `op` names (the bodies — I/O, crypto, syscalls — stay Rust).
- No `Module`/`Plugin` trait scaffolding in Rust; the `HandlModule` /
  `HandlLibraryPlugin` from Phase 1 handles registration.

The Rust that remains is pure implementation: `fn call(&self, args) ->
Result<Value, EvalError>` bodies. No trait boilerplate, no bridge code,
no per-lib plugin scaffolding.

**Done when:** all native libs are migrated per ADR-0083 (FFI bindings,
not Rust bodies); their Rust crates deleted entirely.

### Phase 5 — Capability declaration in handl

Move `CapabilityDecl` from Rust `Language` construction into `.hnl`
source, so a handl library declares its own capability surface.

```handl
-- fs.hnl
cap "fs.read"
cap "fs.write"

op "fs.read" : Path -> Bytes
  ...
```

This requires:

- **Surface syntax** for `cap` declarations in `.hnl`.
- **Loader integration** — the module loader extracts `cap`
  declarations and feeds them into the `Language`'s capability set
  (currently built in Rust via `CapabilityDecl::new`).
- **`HandlLibraryPlugin` integration** — the plugin reads the declared
  capabilities and contributes them via `ContributionContext`.

**Done when:** a handl library's capability set is fully determined by
its `.hnl` source; no Rust `CapabilityDecl::new` calls remain for
migrated libs.

### Phase 6 — ADR-0069 Phase 7: arrows/ as importable modules

Move `handl-packages/` arrow files to be importable `.hnl` modules (ADR-0069
Phase 7). The loader already supports this; the eval wiring from Phase 1
makes the imported arrows callable. Verify the full cascade: atoms →
insert → sort → user programs, all via imports with compositional
inference, under each of the three normalization strategies.

**Done when:** `handl-packages/**/*.hnl` are importable; a user
program can `import sort from "collection/sort"` and call it; inference
composes across the import boundary.

### Multi-target support — interface/impl split

The stdlib is already platform-agnostic by design (ADR-0002). Native
libs follow an **interface/impl** split:

- **Interface crate** (`plugin-lib-X`) — abstract trait, no I/O deps,
  platform-neutral. Defines the API surface.
- **Impl crate** (`plugin-lib-X-std` / `-native` / `-wasm`) — concrete
  backend for a specific target. The host picks which impl to register.

Existing examples:

```
plugin-lib-os          → abstract traits (Fs, Env, Path, Time, Cmd, …)
plugin-lib-os-std      → impl via Rust stdlib (std::fs, std::process)

plugin-lib-http-client → abstract trait (HttpClient)
plugin-lib-http-client-std → impl via ureq

plugin-lib-ws-whatwg   → abstract
plugin-lib-ws-whatwg-native → impl via native syscalls
plugin-lib-ws-whatwg-wasm   → impl via WASM (cfg target_arch = "wasm32")
```

The `op`/`handl` migration preserves and strengthens this split:

- **Interface** → `.hnl` file with `op` declarations + axioms + `cap`
  declarations (platform-neutral, shared across all targets)
- **Impl** → Rust `handl` bodies registered under the `op` names (one
  per target: `std`, `native`, `wasm`, `js`, …)

```handl
-- os.hnl (platform-neutral, shared across all targets)
--- fs.read — read a file as bytes.
--- Contract: IsUtf8(output)
op "fs.read" : Path -> Bytes
  guarantee: IsUtf8(output)

cap "fs.read"
```

```rust
// os-std crate (Rust native)
handl "fs.read" = rust_std_fs_read   // std::fs::read

// os-wasm crate (WASM target)
handl "fs.read" = wasm_fs_read        // browser fetch API

// os-js crate (JS target, future)
handl "fs.read" = js_fs_read          // Node.js fs.readFile
```

The host selects the impl crate based on target. The `.hnl` interface,
axioms, proofs, capabilities, and doc comments are shared across all
targets — only the `handl` bindings change.

**Runtime support:**

- **Interpreted (tree-walk):** works today — `op` dispatches to
  `ModuleLookup`.
- **JIT (cranelift):** `op` lowers to `ArrSource::Foreign` in Scheme IR
  — same dispatch path, different eval engine.
- **WASM:** `ws-whatwg-wasm` already proves the pattern via
  `cfg(target_arch = "wasm32")`.
- **JS:** a future `plugin-lib-X-js` impl crate would register JS-backed
  handls under the same `op` names.
- **Sandbox:** the `cap` declarations + `requires`/`guarantee`
  enforcement (Phase 3) form the sandbox boundary — a sandboxed host
  refuses to register handls for caps it doesn't allow.

The `op`/`handl` split _is_ the target abstraction. The interface
(`.hnl`) is target-neutral; the impl (Rust handls) is per-target. No
additional mechanism is needed for multi-target support — it is
inherent in the design.

## What stays Rust forever

**Updated by [ADR-0083](./0083-ffi-native-stdlib.md):** native function
bodies are **no longer Rust** — they are handl over direct C/POSIX FFI
bindings. The Rust surface is reduced to the kernel only.

- **The `plugin-lib-handl` crate** — the generic loader itself.
- **The `plugin-ffi` crate** (ADR-0083) — the FFI dispatcher.
- **The engine crates** (`plugin-engine-lex-pattern`,
  `plugin-engine-parse-pratt`, `plugin-engine-exec-tree-walk`,
  `plugin-engine-exec-cranelift`, `plugin-engine-exec-bytecode`,
  `plugin-engine-typecheck-nbe`, …) — the language runtime.
- **The `plugin-lib-lang-*` crates** (6 crates) — the self-hosting
  bridge: `eval`, `lex`, `parse`, `compile`, `typecheck`,
  `run_compiled`. These expose the kernel's own engines to handl code
  (e.g. `eval.eval(lang, ast)`). They are irreducible — handl cannot
  eval handl without an evaluator.
- **The host crates** (`host-cli`, `host-lsp`, `host-tooling`) — the
  platform.
- **The core crates** (`handl-language`, `handl-runtime`,
  `handl-value`, `platform`, `plugin-module`, `plugin-loader-native`).

Everything else — logic, declarations, contracts, proofs, capabilities,
doc comments, **and native implementations** — lives in `.hnl` files.

## Consequences

### Positive

- **Massive Rust deletion.** ~60 `plugin-lib-*` crates collapse to ~1
  generic loader crate + N `.hnl` files + a small number of Rust
  `Function` bodies for native primitives.
- **Proofs where Rust can't offer them.** Every public arrow carries an
  axiom and (for handl-authored logic) a compositional proof. Native
  `op`s carry trusted axioms enforced at the boundary.
- **Single source of truth.** A library's logic, types, contracts,
  capabilities, and documentation all live in one `.hnl` file. No Rust /
  handl duplication.
- **ADR-0069 Phase 7 completed.** The arrow-native module system reaches
  its intended end state: stdlib as importable `.hnl` files.
- **Compositional inference end-to-end.** Contracts cache per module;
  inference scales across the import graph without re-tracing.

### Negative / trade-offs

- **`op` axioms are trusted, not proven.** The Rust body (the handl) is assumed
  to honor the declared `guarantee`/`requires`. A bug in the Rust body
  that violates the axiom is not caught by the proof system — only by
  runtime `requires` checking (Phase 3) or testing.
- **Runtime `requires` checking has overhead.** Checking predicates at
  every `Foreign` call adds cost. May need a mode that disables runtime
  checks in production (trusting static inference + the axiom).
- **Two-phase migration.** Pure-logic libs migrate first (Phase 1);
  native libs wait for Phase 2–4. During the interim, the stdlib is a mix
  of `.hnl`-loaded and Rust-registered modules.
- **Surface syntax growth.** `op` and `cap` declarations add
  two new top-level forms to the data language. The Pratt parser and
  lexer must support them.
- **Rust `Function` bodies are now isolated.** Each native `Function`
  impl is a leaf with no trait scaffolding around it. This is simpler but
  loses the `Module`-level grouping in Rust (the grouping moves to
  `.hnl`).

### Compatibility

- **Pure-logic lib migration (Phase 1)** preserves the public API of each
  crate (proven with `plugin-lib-stats`). Downstream crates that depend
  on `plugin-lib-stats::LibraryPlugin` etc. continue to work.
- **Native lib migration (Phase 4)** changes the crate structure: the
  `Module`/`Plugin` trait impls are deleted, replaced by `Function` impls
  - `.hnl` interface. Downstream crates that depend on the `Module`/
    `Plugin` types will need to switch to the `HandlLibraryPlugin` /
    `HandlModule` API. This is a breaking change for native lib crates,
    gated behind Phase 4.
- **`op` and `cap` syntax (Phases 2, 5)** are additive — new
  declaration forms that don't affect existing `.hnl` files.

## Rationale

The stdlib today is mostly Rust scaffolding around handl logic. The
scaffolding exists because the eval-side module registry and the
inference-side module loader were never connected. `HandlModule` (just
built) closes that gap for pure-logic libs. For native libs, the gap is
wider: there is no handl surface syntax for foreign declarations, and
axioms are not enforced at the eval boundary.

This ADR commits to closing both gaps. The payoff is not just Rust
deletion — it is moving the stdlib's correctness story (contracts,
proofs, capabilities) into handl, where it can be verified, and out of
Rust, where it cannot. The Rust that remains is pure implementation,
trusted to honor the contracts that handl declares and enforces.

The phase ordering ensures each step is independently shippable and
reversible. Phase 1 (pure-logic libs) is already in progress and proven.
Phases 2–3 (FFI + axioms) are the enabling work for native lib migration.
Phases 4–5 complete the migration. Phase 6 is cleanup that ADR-0069
already anticipated.

## Related ADRs

- [ADR-0069 — Arrow-Native Modules](./0069-arrow-native-modules.md) — the
  module system this migration targets (Phase 7).
- [ADR-0052 — Scheme IR Contracts](./0052-scheme-ir-contracts.md) — the
  `ArrSource::Foreign` / `Scheme` IR this builds on.
- [ADR-0068 — Predicates as Types](./0068-predicates-as-types.md) — the
  refinement types axioms use.
- [ADR-0051 — FFI Rust Binding Facade](./0051-ffi-rust-binding-facade.md)
  — prior FFI design; this ADR supersedes its Rust-side declaration model
  by moving declarations into handl.
- [ADR-0070 — Refinement Inference Scope](./0070-refinement-inference-scope.md)
  — the inference engine that consumes the contract cache.

## Appendix: Full stdlib inventory and migration map

Every `plugin-lib-*` crate, its category, approximate Rust line count,
and migration path. Categories:

- **Pure** — logic is (or can be) handl-authored; no Rust crate deps.
  Migrates via `HandlModule` (Phase 1).
- **Mixed** — some functions are pure handl, others wrap Rust crates.
  Migrates in two steps: pure functions via `HandlModule` (Phase 1),
  native functions via `op`/`handl` (Phase 4).
- **Native** — all functions wrap Rust crates or syscalls. Migrates via
  `op`/`handl` FFI (Phase 4).
- **Interface** — defines the API surface (module/function names),
  delegates to a `-std` or native variant. Migrates to a `.hnl` file
  with `op` declarations (Phase 4); the delegate crate provides the
  handls.
- **Special** — engine crates, the loader itself. Stays Rust.

### Already migrated

| Crate              | Lines | Status                                       |
| ------------------ | ----- | -------------------------------------------- |
| `plugin-lib-stats` | 390   | Migrated to `HandlModule` (proof-of-concept) |
| `plugin-lib-handl` | 637   | The generic loader itself — stays Rust       |

### Pure-logic (Phase 1)

| Crate                   | Lines | Notes                                                |
| ----------------------- | ----- | ---------------------------------------------------- |
| `plugin-lib-hello`      | 329   | Has `.hnl`, uses `include_str!` — simplest migration |
| `plugin-lib-dedent`     | 268   | String manipulation, no native deps                  |
| `plugin-lib-type`       | 195   | Type introspection helpers, no native deps           |
| `plugin-lib-compress`   | 100   | Interface to compress-gzip                           |
| `plugin-lib-kv`         | 305   | Interface to kv-hashmap                              |
| `plugin-lib-kv-hashmap` | 186   | In-memory key-value store, pure logic                |
| `plugin-lib-encoding`   | 97    | Interface to base64/hex                              |
| `plugin-lib-archive`    | 110   | Interface to archive-tar                             |

### Mixed — pure logic + native ops (Phase 1 for pure parts, Phase 4 for native parts)

| Crate                      | Lines | Native deps             | Notes                                                                                    |
| -------------------------- | ----- | ----------------------- | ---------------------------------------------------------------------------------------- |
| `plugin-lib-collections`   | 4446  | Rust `Vec`/`HashMap`    | Has `.hnl`; array/list/deque/stack/queue/map/set — some pure, some wrap Rust             |
| `plugin-lib-utils`         | 10165 | Rust `Math`, string ops | Has `.hnl`; fn/math/str/obj/type/cmp/bool/option/result/set/mood/parse/serialize — mixed |
| `plugin-lib-cmp`           | 893   | Rust comparison         | Has `.hnl`; comparison logic — mostly pure, some native                                  |
| `plugin-lib-os`            | 3011  | Interface only          | Has `.hnl`; delegates to `os-std` — interface crate                                      |
| `plugin-lib-http-client`   | 853   | Interface only          | Has `.hnl`; delegates to `http-client-std`                                               |
| `plugin-lib-http-server`   | 819   | `hyper`                 | Has `.hnl`; server logic + native HTTP                                                   |
| `plugin-lib-json`          | 180   | `serde_json`            | Has `.hnl`; parse/stringify logic + native serde                                         |
| `plugin-lib-toml`          | 177   | `toml` crate            | Has `.hnl`; parse/stringify + native                                                     |
| `plugin-lib-yaml`          | 179   | `serde_yaml`            | Has `.hnl`; parse/stringify + native                                                     |
| `plugin-lib-crypto`        | 261   | `sha2`/`aes`            | Has `.hnl`; hash/encrypt ops                                                             |
| `plugin-lib-datetime`      | 1228  | `chrono`                | Has `.hnl`; date/time logic + native chrono                                              |
| `plugin-lib-rand`          | 3182  | `rand` crate            | Has `.hnl`; random generation                                                            |
| `plugin-lib-unicode`       | 459   | `unicode` libs          | Has `.hnl`; unicode ops                                                                  |
| `plugin-lib-intl`          | 4957  | `icu`                   | Has `.hnl`; internationalization                                                         |
| `plugin-lib-otlp`          | 984   | `otlp`                  | Has `.hnl`; telemetry                                                                    |
| `plugin-lib-test`          | 1238  | native                  | Has `.hnl`; test framework                                                               |
| `plugin-lib-kv`            | 305   | interface               | Has `.hnl`; delegates to kv-hashmap                                                      |
| `plugin-lib-ws-rfc-memory` | 602   | in-memory               | Has `.hnl`; pure websocket frame logic                                                   |

### Native — all Rust (Phase 4)

| Crate                         | Lines | Native deps         | Notes               |
| ----------------------------- | ----- | ------------------- | ------------------- |
| `plugin-lib-base64`           | 126   | `base64`            | Encode/decode       |
| `plugin-lib-hex`              | 123   | `hex`               | Encode/decode       |
| `plugin-lib-ip`               | 1326  | `ipnet`             | IP/CIDR/socket      |
| `plugin-lib-parse`            | 84    | native              | Parser utilities    |
| `plugin-lib-prim`             | 383   | native              | Primitive ops       |
| `plugin-lib-serialize`        | 151   | `serde`             | Serialization       |
| `plugin-lib-tag`              | 1026  | native              | Tagging system      |
| `plugin-lib-tls`              | 762   | `rustls`            | TLS interface       |
| `plugin-lib-tls-native`       | 533   | `rustls`            | TLS implementation  |
| `plugin-lib-url-rfc`          | 216   | `url`               | RFC URL parsing     |
| `plugin-lib-url-whatwg`       | 217   | `url`               | WHATWG URL parsing  |
| `plugin-lib-archive-tar`      | 281   | `tar`               | Tar archive         |
| `plugin-lib-compress-gzip`    | 224   | `flate2`            | Gzip compression    |
| `plugin-lib-arrow-iter`       | 204   | native              | Arrow iteration     |
| `plugin-lib-concurrent`       | 6622  | `tokio`             | Concurrency         |
| `plugin-lib-os-std`           | 936   | `std::fs`/`process` | OS operations       |
| `plugin-lib-http-client-std`  | 138   | `reqwest`           | HTTP client impl    |
| `plugin-lib-http-server-std`  | 491   | `hyper`             | HTTP server impl    |
| `plugin-lib-otlp-std`         | 468   | `otlp`              | OTLP implementation |
| `plugin-lib-ws-rfc`           | 2407  | native              | WebSocket RFC       |
| `plugin-lib-ws-rfc-native`    | 1505  | native              | WebSocket native    |
| `plugin-lib-ws-whatwg-native` | 423   | native              | WebSocket WHATWG    |
| `plugin-lib-ws-whatwg-wasm`   | 486   | native              | WebSocket WASM      |

### Special — stays Rust (no migration)

| Crate                          | Lines | Notes                          |
| ------------------------------ | ----- | ------------------------------ |
| `plugin-lib-lang-compile`      | 554   | Language engine — compile      |
| `plugin-lib-lang-eval`         | 514   | Language engine — eval         |
| `plugin-lib-lang-lex`          | 513   | Language engine — lex          |
| `plugin-lib-lang-parse`        | 488   | Language engine — parse        |
| `plugin-lib-lang-run-compiled` | 493   | Language engine — run compiled |
| `plugin-lib-lang-typecheck`    | 534   | Language engine — typecheck    |
| `plugin-lib-handl`             | 637   | The generic loader itself      |

### Migration summary

| Category         | Crates | Total Rust lines | Migration phase | Rust after migration     |
| ---------------- | ------ | ---------------- | --------------- | ------------------------ |
| Already migrated | 2      | 1027             | Done            | ~637 (loader only)       |
| Pure-logic       | 8      | ~1590            | Phase 1         | 0 (all `.hnl`)           |
| Mixed            | 18     | ~23900           | Phase 1 + 4     | `op` handls only         |
| Native           | 23     | ~17600           | Phase 4         | `op` handls only         |
| Special          | 7      | ~3733            | Never           | stays Rust               |
| **Total**        | **58** | **~47850**       |                 | **~3733 + handl bodies** |

After migration, ~44,000 lines of Rust scaffolding (Module/Plugin/Function
trait impls, bridge code, per-lib runtime construction) is deleted. What
remains is ~3,733 lines of engine/loader code + the native `Function`
bodies (the handls) + N `.hnl` files carrying all logic, contracts,
proofs, capabilities, and documentation.
