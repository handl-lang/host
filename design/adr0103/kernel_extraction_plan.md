---
title: 'ADR-0103 Phase 2: The Kernel is the Evaluator, Not the Calculus'
type: design
date: 2026-09-10
status_note: Proposed
---

# ADR-0103 Phase 2: The Kernel is the Evaluator, Not the Calculus

## Context

ADR-0098 ("one language, multiple interpreters") established that Handl
should define one data-driven language interpreted by multiple models:
`exec` computes values, `type` computes judgments (HoTT semantics), future
models may compile, optimize, prove, or visualize.

The current implementation falls short of this goal. The `Term` enum in
`crates/plugin-engine/src/term.rs` has 28 variants. Only a handful are
generic arrow-calculus primitives (Hughes arrows). The rest are model
semantics — type/proof/HoTT judgments, data operations, derivable forms —
that leaked into the computational kernel.

Consequences:

- Every execution engine (tree-walk, bytecode, cranelift) carries dead
  `NotImplemented` arms for variants it cannot compute.
- The typechecker (`nbe`) implements the same variants as type judgments.
- The arrow calculus, type semantics, and HoTT semantics are partially
  hardcoded in the kernel enum rather than expressed as data.
- Adding a language construct or type former requires editing Rust, not
  data — violating ADR-0044 ("language as data").

The architectural goal (ADR-0098) is literal: the interpreters should be
data, not enum variants. The kernel should be the infrastructure that runs
interpreters, not the interpreters themselves.

## Decision

### The insight

Interpretation of metasemantics is just computation. The arrow calculus
interpretation, the type model, the HoTT semantics — they are all Handl
code running on a generic execution engine. No special kernel support
needed. No `Term` enum. No calculus variants in Rust.

The arrow calculus is the **first model**, not the kernel. The kernel runs
models. The models are data.

### Architecture

```
Kernel (Rust, irreducible):
  - load language data (yaml: grammar, ast, eval, lexer, tokens)
  - load metalanguage data (hnl: exec_model, arrow_model, type_model)
  - bytecode VM: fetch-decode-execute loop over bytecode
  - hardware boundary: arithmetic (ALU), memory (load/store), ffi (OS/effects)

Exec model (Handl, compiled to bytecode):
  - structural ops: proj_child, eval_block, push_handler, pattern_match, ...
  - eval schemes (from eval.yaml)
  - how to evaluate AST nodes

Arrow model (Handl, compiled to bytecode):
  - what Seq/Fst/Choice/Loop/Op/Handle/App mean

Type model (Handl, compiled to bytecode):
  - what tags mean as type judgments (HoTT)

Meta model (Handl, compiled to bytecode):
  - how to load/compose/run models

Handl data (everything else):
  - language definition (yaml)
  - metalanguage (hnl models)
  - stdlib (hnl)
```

The kernel is a **loader + bytecode VM + hardware boundary**. It loads
language data (how to parse), metalanguage data (how to interpret), and
stdlib data (what's available). Then it runs.

The VM is the execution engine — a loop over bytecode plus the hardware
boundary. The exec model, arrow model, type model, and meta model are all
Handl code compiled to bytecode and run on the VM. The exec model includes
the structural ops (`proj_child`, `eval_block`, `push_handler`, etc.) —
they're not a separate layer, they're how the exec model evaluates AST
nodes. The engine does not hardcode any of it.

### The kernel: bytecode VM, not named ops

The kernel is a **bytecode VM** — a loop over bytecode instructions plus
the hardware boundary. It is NOT a set of named structural ops
(`proj_child`, `eval_block`, `push_handler`, etc.). Those structural ops
are part of the exec model — Handl code compiled to bytecode and run on
the VM.

The kernel has exactly two parts:

#### 1. The bytecode VM loop (Rust)

A fetch-decode-execute loop over a bytecode instruction set. The
instruction set is the irreducible machine — load, store, call, jump,
branch. This is the same complexity as a set of named ops, but expressed
as a loop over bytecode instead of a dispatch table of named functions.
The advantage: the structural ops (`proj_child`, `eval_block`,
`push_handler`, `pattern_match`, `closure`, `apply`) become Handl code in
the exec model, compiled to this bytecode, not Rust functions. The kernel
shrinks to the loop + the hardware boundary.

#### 2. The hardware boundary (Rust)

The irreducible operations that touch the actual machine — physics + OS
boundary. Three things, nothing more:

| Op                                   | What it does             |
| ------------------------------------ | ------------------------ |
| `add`, `sub`, `mul`, `eq`, `lt`, ... | CPU arithmetic (ALU)     |
| `load`, `store`                      | Memory access (RAM)      |
| `ffi`                                | Host/OS syscall boundary |

Everything else is expressible in Handl compiled to bytecode:

- `typeof` → `getfield "__tag__"`
- `alloc`/`drop` → `ffi` to allocator, or `mmap` + Handl GC
- `getfield`/`setfield`/`call`/`ret`/`jump`/`branch`/`make_closure`/
  `pattern_match` → bytecode orchestration over `load`/`store`/`call`

The kernel is three things: arithmetic, memory, ffi. Everything above is
bytecode or Handl.

Everything else — `proj_child`, `eval_block`, `eval_rec`, `proj_env`,
`extend_env`, `closure`, `pattern_match`, `push_handler`, `pop_handler`,
`apply`, `record`, `field_access`, `array`, `tag.construct`,
`arrow_field`, `call_op`, `import`, `export` — is Handl code compiled to
bytecode and run on the VM. They are NOT kernel native ops. They are the
first layer of Handl code on top of the VM.

### Serialization boundary

Native op **implementations** (the VM loop + hardware boundary) are Rust —
opaque, not serializable, not resumable. This is the fixed substrate by
design. Native op **calls** (bytecode instructions) are serializable as
bytecode. The boundary is exact: the VM + hardware boundary is the
non-serializable root; everything above it (Handl source, bytecode,
terms, eval schemes, models, environment state, values) is serializable
data. Computation that crosses into `ffi` (e.g. a file handle) cannot be
resumed — the native state is lost. Computation that stays in Handl
data/bytecode can be paused, serialized, and resumed.

### Why bytecode VM, not named ops

Two paths were considered:

- **A. Named structural ops in Rust.** The kernel is a dispatch table of
  named functions (`proj_child`, `eval_block`, ...). Clear, inspectable.
  Bootstrap is straightforward. But the structural ops are permanently
  Rust — they don't shrink as self-hosting progresses.
- **B. Bytecode VM + hardware boundary.** The kernel is a loop over
  bytecode plus the hardware boundary. Structural ops are Handl compiled
  to bytecode. The Rust kernel shrinks to the loop + hardware. As
  self-hosting progresses, even the compiler becomes Handl. The
  irreducible remainder is the hardware boundary.

**Decision: B.** The bytecode VM is the same complexity as named ops, but
it's the target the system converges to anyway. Starting with B avoids a
future migration from named ops to bytecode. The structural ops become
part of the exec model — Handl code on the VM, not permanent Rust.

### What is NOT a kernel native op

The following are currently native ops in Rust but move out:

**To `exec_model.hnl`** (structural ops, part of the exec model):

- `proj_child`, `proj_child_or`, `proj_rest` — AST recursion
- `eval_block`, `eval_rec` — block/fixpoint evaluation
- `proj_env`, `extend_env`, `closure` — environment management
- `pattern_match`, `push_handler`, `pop_handler`, `apply` — control
- `record`, `field_access`, `array`, `tag.construct`, `arrow_field` — data
- `call_op`, `import`, `export` — host/module dispatch

**To `arrow_model.hnl`** (arrow-calculus combinators, not kernel):

- `first`, `second`, `left`, `right`, `sum`, `product` — arrow combinators
- `app` — arrow application (ArrowApply); calls `apply`
- `tag` — Either routing
- `flow` (`>>>`) — function application
- `route_on_type` — type-tag dispatch

**To `exec_model.hnl`** (exec semantics, not kernel):

- `number_add` — tagged Number arithmetic

**To `type_model.hnl`** (HoTT semantics, not kernel):

- `transport` — path transport

**To Handl library / test harness** (not kernel):

- `is_pos`, `dec`, `inc`, `half_floor` — test helpers
- `inc_fst`, `countdown_step`, `infinite_step` — test helpers
- `assert`, `runtime_assert` — test/graduated infrastructure
- `opt_chain`, `nullish` — derivable sugar

### How computation is represented as data

Each AST node kind has an eval scheme (from `eval.yaml`). Currently these
are `Term` values (`Seq`, `Fst`, `Op`, ...). After this decision, they are
**Handl functions** — data that compiles to bytecode.

Example — eval scheme for `assign`:

```yaml
- id: eval.assign
  node_kind: assign
  scheme: |
    val :: proj_child("value")
    extend_env(proj_child("target"), val)
    val
```

This is Handl code. It calls `proj_child` (structural op, exec model) to
recurse, `extend_env` (structural op, exec model) to bind. The VM runs
the compiled bytecode. No `Term` enum. No Rust match arms.

### The exec model (`exec_model.hnl`)

The exec model defines how to evaluate AST nodes. It includes the
structural ops (`proj_child`, `eval_block`, `push_handler`, `apply`, ...)
and the eval scheme dispatch. These are Handl functions compiled to
bytecode.

```handl
--- exec_model.hnl

export proj_child :: (node, role) ~> do {
  children :: getfield(node, "children")
  idx :: getfield(children, role)
  eval(idx)
}

export extend_env :: (name, val) ~> do {
  push_scope()
  bind(name, val)
  val
}

export eval_block :: (node) ~> do {
  push_scope()
  result :: foreach child in getfield(node, "children") { eval(child) }
  pop_scope()
  result
}

export push_handler :: (handler) ~> do {
  stack :: getfield(ctx, "handler_stack")
  setfield(ctx, "handler_stack", [...stack, handler])
}

export apply :: (callee, args) ~> call(callee, args)

export eval :: (node) ~> do {
  kind :: getfield(node, "kind")
  scheme :: lookup_eval_scheme(kind)
  scheme(node)
}
```

### The arrow calculus model (`arrow_model.hnl`)

Interprets the arrow-calculus tags with Sanada's semantics (algebraic
effects and handlers for arrows, JFP 2024), point-free and serializable:

```handl
--- arrow_model.hnl

export run :: (self, term, incoming, ctx) ~> match getfield(term, "__tag__") {
  "Arr": getfield(term, "value"),

  "Seq": self(self, getfield(term, "snd"),
               self(self, getfield(term, "fst"), incoming, ctx), ctx),

  "Fst": pair(self(self, getfield(term, "inner"), pair_fst(incoming), ctx),
              pair_snd(incoming)),

  "Choice": if is_left(incoming)
            then self(self, getfield(term, "l"), left_val(incoming), ctx)
            else self(self, getfield(term, "r"), right_val(incoming), ctx),

  "Loop": trace(self, getfield(term, "body"), incoming, ctx),

  "Op": do {
    arg_vals :: eval_args(self, getfield(term, "args"), incoming, ctx)
    call_op(getfield(term, "name"), arg_vals, ctx)
  },

  "Handle": do {
    handler :: self(self, getfield(term, "impl_"), incoming, ctx)
    push_handler(handler)
    result :: self(self, getfield(term, "body"), incoming, ctx)
    pop_handler()
    result
  },

  "App": do {
    callee :: self(self, getfield(term, "callee"), incoming, ctx)
    args :: eval_args(self, getfield(term, "args"), incoming, ctx)
    apply(callee, args)
  }
}
```

This is Sanada's arrow calculus (algebraic effects + handlers for arrows),
point-free and serializable. It's a model — data, not Rust.

### The type model (`type_model.hnl`, HoTT semantics)

Interprets the same tags as type/proof judgments:

- `Seq` → type composition
- `Choice` → sum type
- `Op` → graded operation signature
- `Handle` → handler type installation
- `Tag("Path", ...)` → identity type `Id(a, b)`
- `Tag("Equiv", ...)` → equivalence type
- `Tag("Refine", ...)` → subtype
- `Tag("Cobind", ...)` → graded type (coeffect)
- etc.

HoTT is the semantic foundation of the type model:

- Types are spaces (fibrant types, universes).
- Values are points (inhabitants of a space).
- Equality is paths (`Path a b` is identity type `Id_A(a, b)`).
- Equivalence is bijection + paths.
- Rewrites are 2-cells.
- Refinement is a proposition over a type.
- Grades are coeffects.

The type model doesn't "also handle HoTT" — it _is_ the HoTT
interpretation.

```handl
--- type_model.hnl

export run :: (self, term, incoming, ctx) ~> match getfield(term, "__tag__") {
  "Seq": compose_type(self(self, getfield(term, "fst"), incoming, ctx),
                       self(self, getfield(term, "snd"), incoming, ctx)),

  "Choice": sum_type(self(self, getfield(term, "l"), incoming, ctx),
                     self(self, getfield(term, "r"), incoming, ctx)),

  "Path": identity_type(self(self, getfield(term, "from"), incoming, ctx),
                        self(self, getfield(term, "to"), incoming, ctx)),

  "Equiv": equivalence_type(self(self, getfield(term, "a"), incoming, ctx),
                            self(self, getfield(term, "b"), incoming, ctx)),

  "Refine": refine_type(self(self, getfield(term, "predicate"), incoming, ctx),
                        incoming),

  "Cobind": graded_type(self(self, getfield(term, "body"), incoming, ctx),
                        getfield(term, "grade")),

  _: tag("TypeError", {message: "unknown tag: " + getfield(term, "__tag__")})
}
```

### The meta-model (`meta_model.hnl`)

Interprets models themselves. A model is a `Term` value (a record of
handlers). The meta-model can load, validate, compose, and run models:

```handl
--- meta_model.hnl

export load_model :: (path) ~> do {
  source :: ffi("fs", "read", [path])
  parse(source)
}

export compose :: (model_a, model_b) ~>
  (term, incoming, ctx) ~> model_b.run(model_a.run(term, incoming, ctx), ctx)

export run_model :: (model, term, incoming, ctx) ~> model.run(term, incoming, ctx)
```

### The categorical picture

- **Kernel (Rust)** = the bytecode VM + hardware boundary. This is the
  **infrastructure**, not the calculus. It's fixed, minimal, and doesn't
  know what any tag means.
- **Arrow calculus model** = a Freyd category with algebraic effects and
  handlers (Sanada's semantics, point-free). This is the **computational
  model** — one interpretation among many.
- **Type model (HoTT)** = types as spaces, values as points, equality as
  paths. Another interpretation of the same data.
- **Meta-model** = models as data, composable and bootstrappable.

The arrow calculus is not the kernel. It's the first model. The kernel
runs models. "One language, multiple interpreters" is literal: the
interpreters are models, the kernel runs them.

## What disappears from Rust

### `Term` enum — gone

The 28-variant `Term` enum in `crates/plugin-engine/src/term.rs` is
removed. `Term` becomes `serde_json::Value` — tagged records. No Rust
enum variants for `Seq`/`Fst`/`Choice`/`Path`/`Equiv`/etc.

### Engine `match` arms — gone

The tree-walk, bytecode, and cranelift execution engines no longer have
`match` arms for each `Term` variant. The VM runs bytecode. The bytecode
is compiled from Handl code (structural ops, eval schemes, models). The
VM doesn't know what any tag means — it just executes bytecode.

The dispatch is in the bytecode:

```
; eval(term, incoming, ctx, model):
;   tag = term["__tag__"]
;   handler = model[tag]
;   handler(eval, term, incoming, ctx)
;
load model              ; the active model
load term
getfield "__tag__"      ; tag = term["__tag__"
; ... lookup handler in model by tag
; ... call handler with (eval, term, incoming, ctx)
```

### `NotImplemented` arms — gone

There are no `NotImplemented` arms because there are no variant-specific
arms. Unknown tags are a model error, not a kernel error.

### HoTT/proof arms in typechecker — gone

The nbe typechecker's `Path`/`Equiv`/`CellId`/etc. arms move to
`type_model.hnl`. The typechecker becomes a thin dispatcher that calls
the type model.

## What stays in Rust

- The bytecode VM loop (fetch-decode-execute over bytecode)
- The hardware boundary: arithmetic (ALU), memory (load/store), `ffi`
  (OS/effects boundary)
- `Term` as `serde_json::Value` — tagged data

The kernel is three things: arithmetic, memory, ffi. Everything else is
bytecode or Handl.

## Consequences

### Positive

- **Minimal kernel.** The kernel is a bytecode VM + hardware boundary
  (arithmetic, memory, ffi). Everything else is data.
- **True "one language, multiple interpreters."** The interpreters are
  models (Handl data), not enum variants.
- **No `NotImplemented` arms.** The VM runs any bytecode; unknown tags
  are model errors.
- **Extensibility.** Adding a type former, model, or language construct is
  a data change, not a Rust edit.
- **Self-hosting path.** The arrow calculus, type model, and meta-model
  are Handl code. They can be compiled, optimized, and eventually
  self-hosted. The kernel shrinks toward the hardware boundary.
- **Serializable.** `Term` is JSON. Bytecode is serializable. Computation
  that stays in Handl data/bytecode can be paused, serialized, resumed.
- **Effect boundary.** `ffi` is the only way to reach effects — I/O,
  syscalls, network, files, time, randomness. Without `ffi`, the VM is
  pure computation over memory.

### Negative

- **Performance.** Dispatching through a model (Handl arrow) is slower
  than Rust match arms. Bytecode/cranelift may need to compile the
  model's handlers into native code for performance.
- **Bootstrapping.** The evaluator needs a model to interpret tags. The
  first model is `arrow_model.hnl`. It must be loaded before the
  evaluator can interpret arrow-calculus terms.
- **Migration cost.** The 28-variant `Term` enum, all engine match arms,
  and the typechecker's HoTT arms must be moved to models. This is a
  large refactor.

## Migration Plan

### Phase 1: Define the VM instruction set + hardware boundary

Define the bytecode instruction set (push/pop/load/store/getfield/setfield/
call/ret/jump/branch/make_closure/pattern_match) and the hardware boundary
(arithmetic, memory, ffi). Implement the VM loop in Rust. This replaces
the current tree-walk engine's dispatch core.

### Phase 2: Make eval schemes Handl-callable

Convert `eval.yaml` schemes from `Term` values to Handl functions calling
structural ops. An eval scheme for `assign` becomes Handl code that calls
`proj_child("value")` and `extend_env(...)`. These compile to bytecode.

### Phase 3: Author the arrow calculus model

Author `arrow_model.hnl` with the 8 arrow-calculus handlers (`Arr`, `Op`,
`Seq`, `Fst`, `Choice`, `Loop`, `App`, `Handle`). This is Sanada's
calculus, point-free and serializable. Compiles to bytecode.

### Phase 4: Strip execution engines to the VM

Remove the `Term`-variant match arms from tree-walk, bytecode, cranelift.
The engines become the VM + exec model. No hardcoded semantics.

### Phase 5: Author the type model (HoTT)

Author `type_model.hnl` with HoTT interpretations of all tags (kernel tags

- `Path`/`Equiv`/`CellId`/`Refine`/`Cobind`/etc.). Move the nbe
  typechecker's HoTT arms into `type_model.hnl`.

### Phase 6: Remove the `Term` enum

Replace the 28-variant `Term` enum with `serde_json::Value` (tagged
records). Update all constructors and pattern matches.

### Phase 7: Author the meta-model

Author `meta_model.hnl` that interprets models as data. Models can be
loaded, validated, composed, and run inside the language.

### Phase 8: Verify

1. `exec_model_parity.rs` — update to compare exec model vs arrow model.
2. `option_b_frontier_e2e.rs` — should be unaffected (surface syntax).
3. `path_transport.rs`, `rewrite_law.rs` — update to use `Tag` data.
4. `nbe/tests.rs` — update to use type model.
5. Full workspace test suite.

### Phase 9: Testing and documentation

**Rust (kernel + VM):**

- 100% test coverage on all public Rust code (VM loop, hardware boundary,
  loader, `Term` as `serde_json::Value`).
- Unit tests for every VM instruction.
- Integration tests for the VM running compiled bytecode.
- Property tests for arithmetic, memory, ffi dispatch.

**Handl (models):**

- 100% test coverage on all models (`exec_model.hnl`, `arrow_model.hnl`,
  `type_model.hnl`, `meta_model.hnl`).
- Every exported arrow has test cases covering its contract.
- Parity tests: exec model vs arrow model, exec model vs type model.
- The factorial example (`factorial(5) = 120`) runs through exec model
  (evaluation) and type model (checking) with the same source.

**Documentation:**

- Doc comments on all public Rust items (VM instructions, hardware
  boundary ops, loader, `Term` type).
- Doc comments on all exported Handl arrows in every model.
- Every model file starts with a module doc comment explaining what it
  interprets and how.

## Resolved Decisions

1. **`app` — model only, kernel has `apply`.** `apply` is a structural op
   on the VM (exec model, not Rust). The arrow-calculus `App` (ArrowApply)
   is a model op in `arrow_model.hnl` that calls `apply`.

2. **`eval_block`/`eval_rec` — exec model.** Both manage the scope stack.
   They are Handl code compiled to bytecode, not Rust primitives. The VM
   auto-pops scopes after each eval-scheme step (kernel manages cleanup on
   failure — no `try/finally` primitive needed in the metalanguage).

3. **`call_host` vs `ffi` — split.** `ffi(module, name, args)` is a
   hardware-boundary op (Rust) — pure host function call with no handler
   fallback. `call_op(name, args)` is a structural op (exec model) —
   handler-interceptable dispatch. The old `call_host` (which had an
   op-decl fallback) is replaced by these two separate ops.

4. **`match` — exec model with kernel primitive.** The `match` op is
   exec model. The kernel provides `pattern_match(pattern, value)` as a
   VM primitive (the 406-line pattern-matching logic stays in Rust as the
   hardware boundary). The `match` op orchestrates arm iteration using
   this primitive.

5. **`handle` — VM primitives + model combinator.** The VM provides
   `push_handler`/`pop_handler` as structural ops (exec model). The
   arrow-calculus `Handle` combinator is a model op built on them. No
   circularity: the VM primitives are stack management; the model
   combinator is calculus semantics.

6. **Cleanup on failure — VM manages.** The VM auto-pops scopes and
   handlers on failure. No `try/finally` primitive needed in the
   metalanguage.

7. **`transport`/`number_add` — move to models.** `transport` moves to
   `type_model.hnl` (HoTT path transport). `number_add` moves to
   `exec_model.hnl` (tagged Number arithmetic). They are model semantics,
   not kernel primitives.

8. **Test helpers — Handl library.** `is_pos`/`dec`/`inc`/`half_floor`/
   `inc_fst`/`countdown_step`/`infinite_step`/`assert`/`runtime_assert`/
   `opt_chain`/`nullish` become Handl library code or test-harness
   registrations, not kernel native ops.

9. **Kernel = bytecode VM + hardware boundary (not named ops).** The
   kernel is a fetch-decode-execute loop over bytecode plus the hardware
   boundary: arithmetic (ALU), memory (load/store), `ffi` (OS/effects
   boundary). Three things. `typeof` → `getfield "__tag__"`. `alloc`/`drop`
   → `ffi` to allocator or `mmap` + Handl GC. Structural ops (`proj_child`,
   `eval_block`, `push_handler`, `pattern_match`, `closure`, `apply`, ...)
   are Handl code compiled to bytecode and run on the VM — not Rust
   functions. This is the target the system converges to as self-hosting
   progresses; starting here avoids a future migration from named ops to
   bytecode.

## Open Questions

1. **Performance** — dispatching through a model (Handl arrow) is slower
   than Rust match arms. Bytecode/cranelift may need to _compile_ the
   model's handlers into native code for performance. Is this acceptable,
   or do we keep hot-path match arms in Rust as an optimization?

2. **Bootstrapping** — the evaluator needs a model to interpret tags. The
   first model is `arrow_model.hnl`. How is it loaded? Options:
   - (a) Hardcode the arrow model in Rust (temporary bootstrap).
   - (b) Load `arrow_model.hnl` from disk at startup.
   - (c) Embed `arrow_model.hnl` as a compiled bytecode image.

3. **Refinement engine** — `plugin-ext-refinement` uses `Term::Rewrite`
   extensively. With `Term` as JSON, `Rewrite` is
   `{"__tag__": "Rewrite", ...}`. The refinement engine pattern-matches on
   the tag. This works, but the refinement engine's static analyses need
   to handle JSON terms.

## Files Affected

### Kernel (Rust — minimal)

- `crates/plugin-engine/src/term.rs` — replace enum with
  `serde_json::Value`
- `crates/plugin-engine/src/lib.rs` — evaluator (dispatch + recurse)
- `crates/plugin-engine-exec-tree-walk/src/lib.rs` — thin dispatcher
- `crates/plugin-engine-exec-bytecode/src/compile.rs` — thin dispatcher
- `crates/plugin-engine-exec-cranelift/src/compile.rs` — thin dispatcher
- `crates/plugin-engine-typecheck-nbe/src/lib.rs` — thin dispatcher

### Models (Handl-authored data)

- `project/design/adr0103/arrow_model.hnl` — arrow calculus (new)
- `project/design/adr0103/exec_model.hnl` — exec model (update)
- `project/design/adr0103/type_model.hnl` — type model, HoTT (new)
- `project/design/adr0103/meta_model.hnl` — meta-model (new)

### Language analysis

- `crates/handl-language/src/law_inference.rs` — update for JSON terms
- `crates/handl-language/src/effects.rs` — update for JSON terms
- `crates/handl-language/src/capabilities.rs` — update for JSON terms

### Refinement

- `crates/plugin-ext-refinement/src/*` — update for JSON terms

### Tests

- `crates/handl-runtime/tests/exec_model_parity.rs`
- `crates/plugin-engine-exec-tree-walk/tests/path_transport.rs`
- `crates/plugin-engine-exec-tree-walk/tests/rewrite_law.rs`
- `crates/plugin-engine-typecheck-nbe/src/tests.rs`

## Result

The `Term` enum disappears. The kernel is a bytecode VM + hardware boundary
(a fetch-decode-execute loop + `ffi`/arithmetic/memory). The exec model,
arrow calculus, type model (HoTT), and meta-model are all Handl code
compiled to bytecode and run on the VM.

```
Kernel (Rust, irreducible):
  bytecode VM loop + hardware boundary (ffi, arithmetic, load/store)

Models (Handl data, compiled to bytecode):
  exec_model.hnl     — how to evaluate (structural ops + eval schemes)
  arrow_model.hnl    — Sanada's arrow calculus (point-free)
  type_model.hnl     — HoTT interpretation
  meta_model.hnl     — models as data
```

"One language, multiple interpreters" is literal. The interpreters are
models. The kernel runs them. The arrow calculus is a model, not the
kernel. The exec model is a model. The kernel is the VM + hardware
boundary — the irreducible machine.
