# Multi-shot `resume` for `handle` effects — design + tree-walk prototype
- Refs: ADR-0158, ADR-0106


**Status**: Proposed (tree-walk prototype landed; cranelift replay driver landed)
**Refs**: ADR-0158 (resumption semantics — `perform` dispatches in-flight),
ADR-0106 (self-contained arrow values), ADR-0076 (`handle` dynamic extent),
Sanada — *Algebraic effects and handlers for arrows*.

## The feature

Handler clauses may take a second parameter — the **resumption**:

```hnl
handle f with {
  choose: (opts, resume) ~> list.flat_map(opts, (o) ~> resume(o))
}
```

`resume(x)` feeds `x` back as the `perform`'s result and continues the
handled body from the op site. Calling it more than once runs the
continuation per call — nondeterminism (`choose`), generators (`yield`),
backtracking.

## The problem

ADR-0158 made `perform` dispatch in-flight (one-shot): `h[op](arg)`'s
return value is the `perform` result — "resume" is the call returning.
A `resume` **value** reifies "the rest of the body's evaluation" as a
callable — a delimited continuation. In the tree-walk engine
(`hnl-engine-exec-tree-walk`) the continuation at a `perform` site is the
Rust call stack inside `eval_inner_td` — not a value.

Reification options considered:

| Option | Verdict |
|---|---|
| **(a) CPS the eval at perform sites** | Rejected — `eval_inner_td` is a non-CPS recursive interpreter over `TermData`; CPS'ing every `Seq`/`Choice`/`App`/`Op` arm is a full-eval rewrite, and it would not survive host-callback boundaries (`TreeWalkCallable` re-enters on a fresh engine). |
| **(b) Coroutine/generator for the body** | Rejected — a stackful coroutine (thread or `gen`) resumes **once**; multi-shot needs *copyable* continuations. Stable Rust has no `gen`, and engine state (`&Ast`, `&mut ScopeStack`) is borrowed, not `Send`. |
| **(c) Delimited re-run (replay journal)** | **Chosen.** `resume(x)` re-evaluates the body term from the `handle` boundary, replaying recorded op results up to the suspension index and feeding `x` at that index. Copyable by construction — every `resume` call gets a fresh run. Works today, no eval rewrite. |
| **(d) Closure over `eval(remaining_term, env)`** | Rejected — eval is not compositional at perform sites: `Seq` recurses into `fst`/`snd` inside the same Rust frame; there is no "next node pointer" to close over. |

## Why replay fits this engine

- **Arrows are self-contained data** (ADR-0106): a body's `TermData`, the
  `incoming` value, the handler value, and the `Ast` arena are all
  serializable data — no process-local frame references.
- **`ScopeStack` is clonable**: the entry environment at the `handle`
  boundary is a snapshot (`scopes` + `captured` + shared `globals` Rc).
- **Ops are already data**: `perform` mints `IO<{op: arg}>` values; the
  journal only has to record the *result fed back* per suspension point.
- **Suspension = the existing error channel**: `LangError::Suspended{id}`
  unwinds to the nearest journal driver; `RefCell` borrows drop cleanly
  on `Result` propagation (unlike `catch_unwind`, which the
  `stacker::maybe_grow` seams would abort on — panics cannot unwind
  through `psm::on_stack`'s `extern "C"` boundary).

## Semantics

Clause arity selects the mode; both forms desugar to the same driver:

- **One-shot** `op: x ~> e` — today's direct-style dispatch. Equivalent
  to `(x, k) ~> k(e)`: the clause result feeds back at the `perform`
  site and the body continues. Unchanged code path.
- **Multi-shot** `op: (x, k) ~> e` — the op **suspends**: eval unwinds to
  the `handle` driver, which invokes the clause with `(payload, resume)`.
  The **clause's result is the `handle` result** (standard deep-handler
  reading — `resume` already folds in the `return` clause).

`resume(x)`:

- re-runs the body under the same handler with the journal prefix
  `entries[0..i] ++ [x]` (entries are `(op, result)` pairs in perform
  order; replay validates the op name at each index and errors on a
  nondeterministic sequence);
- returns the body's completed value — applying the handler's `return:`
  clause when present;
- a suspension *inside* a re-run invokes the covering clause again and
  its result becomes this `resume`'s return value — so
  `flat_map(opts, o ~> resume(o))` collects one result per option
  (delimited `shift`/`reset` semantics: `k` returns the final answer of
  the delimited computation, not the perform-site value).

`resume` is a **copyable, non-consumable** value. One-shot is a style
(call it once), not a type — `k` may be called zero or more times. A
consumable `k` would rule out `choose`/`yield` and adds no safety under
replay (a second call simply re-runs again).

**Escape**: allowed and well-defined for pure bodies — `resume` is a
function of the delimited continuation, so calling it after `handle`
returns re-runs the body. Caveats (documented, not enforced): host
calls and `assign` writes inside the body prefix re-execute on every
`resume` call; the body evals on an env snapshot, so `assign` writes to
enclosing scopes do not escape the handled body (an isolation property
replay *requires*, not just permits).

**Coverage**: `resume` mints only when the op's covering clause takes
≥2 params. Uncovered ops mint residual `IO<{op: arg}>` data as today;
1-ary clauses take the existing one-shot path.

## Mechanism (tree-walk)

```
handle <body> with <handler>
  → eval impl_ → handler_val
  → if any clause is ≥2-ary:  journalize + drive
      push journal {body, incoming, entry_env, handler, ast, entries:[], cursor:0}
      push handler
      eval body
        perform(op, arg) [call_host/call_host_fn intercept]:
          innermost journal whose handler covers `op` with ≥2-ary impl:
            i = cursor++
            i < entries.len()  → return entries[i].result   (replay)
            else               → record suspension{i, op, arg, journal}
                                 → Err(LangError::Suspended{id})
        LangError::Suspended{id} at driver:
          clause = handler.impl[op];  k = {"__resume__": id}
          return apply(clause, [arg, k])
  → else: today's path (guard + eval + interpret_io_former + carried handler)
```

- **`resume` value**: `{"__resume__": <id>}` object; `apply_app`
  intercepts it before the dispatch-object/`__term__`/Arrow arms and
  drives a re-run. Same reserved-key convention as `__term__`/`__arr__`.
- **State lives in thread-locals**, not on the engine: `TreeWalkCallable`
  (host fns calling arrows — e.g. `list.flat_map`'s callback) constructs
  a *fresh* `TreeWalkEngine`, so journal/suspension state on `self`
  would be invisible to `resume` invoked inside the clause's callbacks.
  `RESUME_JOURNALS` (stack, innermost-last) + `RESUME_SUSPENSIONS`
  (id → suspension) + `RESUME_NEXT_ID`.
- **`Term::Op`/`dispatch_op`**: a ≥2-ary impl covering the op under an
  active journal suspends identically (dead code today — 2-ary impls
  applied with one arg error on arity).
- **Suspension ownership**: a `Suspended{id}` records its journal; a
  driver catches only suspensions belonging to *its* journal — outer
  suspensions propagate through nested handles untouched.

## Known limits (prototype)

- **Deterministic bodies only**: replay assumes the op sequence is
  stable across runs. Non-op side effects (host calls, `assign`,
  `enclosing_updated` feedback wires) re-execute per `resume` call —
  an error message names the op/index on replay mismatch, but mutation
  in the prefix is the caller's responsibility.
- **Host-callback boundary**: a suspension crossing `call_value`
  (`fn.map` of a performing arrow, etc.) stringifies at the
  `EvalError` boundary — `resume` *calls* inside callbacks work (they
  drive locally); arrows that *perform* inside host callbacks do not
  suspend through the callback.
- **`.hnl` `try`/error-catch** can swallow `LangError::Suspended` like
  any error — same caveat as any error-channel control flow; the
  `Handle` arm's impl-fallback catch is audited to re-propagate.
- **`handle_dyn` shared op** carries the compiled-backend path: the
  cranelift engine overrides `NativeOpCtx::run_delimited` to install a
  journal when the handler has a ≥2-ary clause, and intercepts `perform`
  / `arr <op>` before one-shot dispatch. Because cranelift arrow values
  run in the tree-walk interpreter (`apply_value_treewalk`), a `resume`
  called from inside a clause body crosses engines: the suspending engine
  registers an engine-neutral driver
  (`native_ops::register_resumption_driver`) and every engine's
  `__resume__` seam consults that shared table before its own. Cranelift
  recovers `&mut EvalCtx` from a thread-local installed for the duration
  of the clause application — a `resume` that *escapes* the clause and is
  called with no cranelift evaluation active errors instead of re-running.
- **Escape** is supported while the eval is live: a clause may return the
  resumption value and the caller may apply it (`k` is first-class and
  copyable). The *cross-engine* bridge needs a cranelift evaluation to be
  active, so a `resume` applied later from a host callback with no
  cranelift eval in progress errors instead of re-running.
- **Eager sibling evaluation** (compiled backends): a body keeps
  evaluating sibling nodes after the first suspension (the deferred-error
  channel), so the journal freezes on the first suspension, re-raising it
  for later `perform`s in the same run — the cursor does not advance past
  it and the first suspension stays the run's result. Cranelift's codegen
  also unwinds immediately on a pending suspension at operand and `Seq`
  boundaries, so this is a belt-and-braces guard.
- **`perform` inside a `rec`/loop fixpoint region** is unsupported — not
  because the suspension is lost (the fixpoint now unwinds to the driver
  instead of skipping the binding), but because a fixpoint re-evaluates
  its statements once per pass, so the same `perform` would consume a new
  journal entry each pass and replay can never converge. Keep performing
  ops outside `rec`/loop desugars.
- **Table lifecycle**: suspensions and cross-engine drivers are retained
  for the duration of an eval (that is what makes an escaped `resume`
  callable) and reclaimed by `ResumeTableScope` when the outermost eval on
  the thread returns.

## Bytecode / Cranelift capture strategy

The bytecode engine implements the replay driver natively (its `apply_app`
re-enters the VM, so journal and clauses stay in one engine). Cranelift
takes the replay path through the shared `handle_dyn` hook + the
cross-engine driver bridge described above; a frame-serialization or
copyable-stack capture remains a later option if profiling demands it:

1. **Frame serialization**: the bytecode VM's eval already materializes
   activation state per arrow application; capture = serialize the
   live frame chain at `perform` into a `Resume` value, restore on
   apply. Cost: every `Op`/`call` needs a snapshot point.
2. **Segmented stacks**: the `stacker` growable-stack seam already
   segments the native stack; a `corosensei`-style stackful-coroutine
   crate gives one-shot for free, but multi-shot still needs a
   copyable-stack implementation (fork the segment chain). Replay
   avoids all of it — recommend replay first for parity, frame
   serialization later if profiling demands it.

## Correspondence (Sanada → this design)

| Sanada | handl + this design |
|---|---|
| clause continuation `k` | `resume` — a `{"__resume__": id}` value |
| `k` invoked once (one-shot) | 1-ary clause sugar `(x) ~> e` ≡ `(x,k) ~> k(e)` |
| `k` multi-shot | `resume` reusable — replay makes it copyable |
| handler = algebra homomorphism | clause result = handle result; `return:` clause = unit map |
| prompt/delimiter | `handle` boundary = `reset`; `perform` = `shift` |
