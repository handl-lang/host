---
title: 'ADR-0103 Phase 1: VM Instruction Set + Hardware Boundary Design'
type: design
date: 2026-09-10
---

# ADR-0103 Phase 1: VM Instruction Set + Hardware Boundary Design

## Status

Design specification — Phase 1 of the ADR-0103 migration plan.

## References

- [ADR-0103: The Kernel is the Evaluator, Not the Calculus](../../adrs/0103-kernel-is-evaluator-not-calculus.md)
- [ADR-0103: Handl as Its Own Metalanguage](../../adrs/0103-handl-metalanguage.md)
- [Kernel Extraction Plan](kernel_extraction_plan.md)
- [Exec Model (`exec_model.hnl`)](exec_model.hnl)
- [Arrow Model (`arrow_model.hnl`)](arrow_model.hnl)
- Current ISA: `crates/plugin-engine-exec-bytecode/src/instr.rs`
- Current VM: `crates/plugin-engine-exec-bytecode/src/vm.rs`
- Pattern matching: `crates/plugin-engine-exec/src/pattern_match.rs`

---

## 1. The ADR-0103 Vision

### 1.1 The kernel is a bytecode VM + hardware boundary

ADR-0103 establishes that the Peregrin/Handl kernel is **not** a set of
named structural ops (`proj_child`, `eval_block`, `push_handler`, ...).
The kernel is exactly two things:

1. **A bytecode VM loop** (Rust) — a fetch-decode-execute loop over a
   minimal instruction set. The instruction set is the irreducible
   machine: load, store, call, jump, branch, make_closure, pattern_match.

2. **A hardware boundary** (Rust) — three categories of operations that
   touch the actual machine, nothing more:

   | Op                                   | What it does             |
   | ------------------------------------ | ------------------------ |
   | `add`, `sub`, `mul`, `eq`, `lt`, ... | CPU arithmetic (ALU)     |
   | `load`, `store`                      | Memory access (RAM)      |
   | `ffi`                                | Host/OS syscall boundary |

Everything else — structural ops, eval schemes, arrow-calculus combinators,
type judgments, HoTT semantics — is **Handl code compiled to bytecode**
and run on the VM. The kernel does not know what any tag means. It just
executes bytecode.

### 1.2 The layering

```
┌─────────────────────────────────────────────────────────┐
│  Handl data (everything else)                           │
│  - language definition (yaml: grammar, ast, eval, ...)  │
│  - stdlib (hnl)                                          │
│  - user programs (hnl)                                   │
├─────────────────────────────────────────────────────────┤
│  Models (Handl, compiled to bytecode)                    │
│  - exec_model.hnl    — structural ops + eval dispatch    │
│  - arrow_model.hnl   — Sanada's arrow calculus            │
│  - type_model.hnl    — HoTT interpretation                 │
│  - meta_model.hnl    — models as data                    │
├─────────────────────────────────────────────────────────┤
│  Kernel (Rust, irreducible)                              │
│  - bytecode VM loop (fetch-decode-execute)               │
│  - hardware boundary (arithmetic, memory, ffi)           │
│  - scope stack + handler stack (VM-managed)              │
│  - pattern_match primitive (kernel primitive)            │
└─────────────────────────────────────────────────────────┘
```

The arrow calculus is the **first model**, not the kernel. The kernel
runs models. "One language, multiple interpreters" is literal: the
interpreters are models (Handl data), the kernel runs them.

### 1.3 Why bytecode VM, not named ops

Two paths were considered (ADR-0103 §"Why bytecode VM, not named ops"):

- **A. Named structural ops in Rust.** The kernel is a dispatch table of
  named functions. Clear, inspectable. But the structural ops are
  permanently Rust — they don't shrink as self-hosting progresses.
- **B. Bytecode VM + hardware boundary.** The kernel is a loop over
  bytecode plus the hardware boundary. Structural ops are Handl compiled
  to bytecode. The Rust kernel shrinks to the loop + hardware.

**Decision: B.** The bytecode VM is the same complexity as named ops, but
it's the target the system converges to anyway. Starting with B avoids a
future migration from named ops to bytecode. The structural ops become
part of the exec model — Handl code on the VM, not permanent Rust.

---

## 2. Target Minimal VM Instruction Set

The target ISA is the irreducible set of bytecode instructions that the VM
loop executes. These are the instructions the Handl compiler targets. The
ISA has approximately 15 instructions, grouped into five categories.

### 2.1 Stack operations

| Instruction   | Stack effect     | Description                                    |
| ------------- | ---------------- | ---------------------------------------------- |
| `Push(value)` | `-> val`         | Push a literal `Value` onto the operand stack. |
| `Pop`         | `val ->`         | Discard the top of the operand stack.          |
| `Dup`         | `val -> val val` | Duplicate the top of the operand stack.        |
| `Swap`        | `a b -> b a`     | Swap the top two values on the operand stack.  |

These are the fundamental stack-manipulation primitives. `Push` is the
only instruction that introduces a literal value. `Pop`, `Dup`, and
`Swap` rearrange the stack without touching memory or the scope stack.

### 2.2 Memory / variable operations

| Instruction         | Stack effect     | Description                                                                                                                      |
| ------------------- | ---------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `Load(name)`        | `-> val`         | Look up `name` in the scope stack and push its value. If unbound, push the name as a string (matching the tree-walk convention). |
| `Store(name)`       | `val ->`         | Pop a value, push a new scope, and bind `name` to the value in that scope. (This is `extend_env`.)                               |
| `LoadField(field)`  | `obj -> val`     | Pop a record/object, push the value of `field` from it. (This is `getfield`.) Returns `null` if the field is absent.             |
| `StoreField(field)` | `obj val -> obj` | Pop a value and a record/object, set `field` to the value on the record, push the updated record. (This is `setfield`.)          |

`Load` and `Store` operate on the **scope stack** (variable bindings).
`LoadField` and `StoreField` operate on **record/object values** on the
operand stack (field access). The distinction matters: `Load`/`Store`
manage the VM's scope stack; `LoadField`/`StoreField` manipulate data
values.

> **Note on `Store` semantics:** `Store(name)` pushes a new scope and
> binds the name. This is the `extend_env` structural op. The matching
> scope pop is handled by `PopScope` (see §2.6) or by the VM's auto-pop
> on failure (see §7.3). For `assign` semantics (update-or-define
> without pushing a scope), the exec model uses a separate `Assign(name)`
> instruction (see §2.6).

### 2.3 Control flow

| Instruction                 | Stack effect                  | Description                                                                                                                                |
| --------------------------- | ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| `Call(n)`                   | `callee arg1..argN -> result` | Pop `n` argument values, pop the callee, apply (closure or host fn), push the result.                                                      |
| `Return`                    | `val ->`                      | Pop the top of the operand stack and return it as the program result.                                                                      |
| `Jump(target)`              | `->`                          | Unconditionally set the instruction pointer to `target`.                                                                                   |
| `Branch(target)`            | `bool ->`                     | Pop a boolean; if true, jump to `target`; if false, fall through.                                                                          |
| `MakeClosure(params, body)` | `-> closure`                  | Capture the current environment and push a closure value with the given parameter names and body (an instruction-range or AST node index). |

`Call(n)` is the universal function-application primitive. The callee
may be a closure value (`{"__arrow__": true, ...}`) or a string naming a
registered host function. The arrow-calculus `App` combinator and the exec
model's `apply` structural op both compile to `Call`.

`MakeClosure` captures the current scope stack environment. When the
closure is later applied via `Call`, the VM pushes a new scope, binds the
parameters, evaluates the body in the captured environment extended with
the parameter bindings, then restores the original scope stack.

`Branch(target)` pops a boolean and jumps if **true**. This is the
conditional jump. The `if`/`else` construct, `Choice` routing, and loop
conditionals all compile to `Branch` + `Jump`.

### 2.4 Hardware boundary

These are the **only** instructions that touch the actual machine. Without
them, the VM is pure computation over memory.

| Instruction            | Stack effect           | Description                                                                         |
| ---------------------- | ---------------------- | ----------------------------------------------------------------------------------- |
| `Arith(op)`            | `a b -> result`        | Pop two values, apply the ALU operation `op`, push the result.                      |
| `Ffi(module, name, n)` | `arg1..argN -> result` | Pop `n` argument values, call the host/OS function `module::name`, push the result. |

#### `Arith(op)` — ALU operations

The `op` field selects the arithmetic operation. The full set:

| Category   | Operations                               |
| ---------- | ---------------------------------------- |
| Arithmetic | `add`, `sub`, `mul`, `div`, `mod`, `neg` |
| Comparison | `eq`, `neq`, `lt`, `gt`, `lte`, `gte`    |
| Logic      | `and`, `or`, `not`                       |

`neg`, `not` are unary (pop one value). All others are binary (pop two
values). These map directly to CPU ALU operations — they are the
irreducible arithmetic boundary.

> **Note:** `Arith` operates on raw values (numbers, booleans). Tagged
> Number arithmetic (untag, add, re-tag) is exec-model semantics
> (`number_add` in `exec_model.hnl`), not a kernel primitive. The kernel
> only provides raw `add`; the tagging/untagging is Handl orchestration.

#### `Ffi(module, name, n)` — Host/OS syscall boundary

`Ffi` is the **only** way to reach effects — I/O, syscalls, network,
files, time, randomness. Without `Ffi`, the VM is a pure computation over
memory: it can load, store, compute, and branch, but it cannot observe
or affect the outside world.

The `module` and `name` fields identify the host function. The host
runtime maintains a registry of `module::name` → Rust function mappings.
Examples:

| Module   | Name        | What it does                                         |
| -------- | ----------- | ---------------------------------------------------- |
| `io`     | `print`     | Write to stdout                                      |
| `fs`     | `read`      | Read a file                                          |
| `fs`     | `write`     | Write a file                                         |
| `net`    | `connect`   | Open a network connection                            |
| `time`   | `now`       | Get the current time                                 |
| `random` | `next`      | Generate a random value                              |
| `native` | `<op_name>` | Call a registered native op (fallback for `call_op`) |
| `module` | `load`      | Load a Handl module by spec                          |
| `alloc`  | `alloc`     | Allocate memory (or use `mmap` + Handl GC)           |
| `alloc`  | `drop`      | Free memory                                          |

`Ffi` is the hardware boundary. Everything above it is bytecode or Handl.

### 2.5 Pattern matching (kernel primitive)

| Instruction    | Stack effect                | Description                                                                                  |
| -------------- | --------------------------- | -------------------------------------------------------------------------------------------- |
| `PatternMatch` | `value pattern -> bindings` | Match `value` against `pattern`. On success, push a binding record. On failure, push `null`. |

Per ADR-0103 resolved decision #4, the `match` op is exec model (Handl
code that orchestrates arm iteration), but the kernel provides
`pattern_match(pattern, value)` as a VM primitive. The 406-line
pattern-matching logic in `crates/plugin-engine-exec/src/pattern_match.rs`
stays in Rust as a kernel primitive.

The six pattern forms (from `pattern_match.rs`):

1. **Wildcard** — `var` node named `_` or `_*` → always matches.
2. **Binder** — `var` node with a name → binds name to scrutinee, always
   matches.
3. **Literal** — any other node → evaluate and compare by `==`.
4. **Array destructuring** — `array` node with sub-pattern children +
   optional `rest` node → positional match, `...rest` captures tail.
5. **Object destructuring** — `object` node with `entry` children +
   optional `rest` node → key match, `...rest` captures unmatched keys.
6. **Tag pattern** — `call` node with a `var` callee (tag name) and `var`
   arg nodes (binders): `ok(v)`, `err(e)`. Dispatches on `__tag__`
   identity and binds args to payload fields.

The `match` op in `exec_model.hnl` calls `PatternMatch` for each arm's
pattern until one succeeds, then evaluates the arm body in the resulting
bindings.

### 2.6 Scope stack management

| Instruction    | Stack effect | Description                                                                                                                   |
| -------------- | ------------ | ----------------------------------------------------------------------------------------------------------------------------- |
| `PopScope`     | `->`         | Pop the innermost scope from the scope stack.                                                                                 |
| `Assign(name)` | `val ->`     | Pop a value and update-or-define `name` in the scope stack (update nearest binding, or define in innermost scope if unbound). |
| `PushScope`    | `->`         | Push a new empty scope (record a mark).                                                                                       |

`PopScope` is the counterpart to `Store(name)` — after a `Seq` whose `fst`
pushed a scope via `Store`, the `snd` completes and `PopScope` removes
the scope. The VM also auto-pops scopes on failure (see §7.3).

`Assign(name)` implements the update-or-define semantics needed for
`while`/`do-while`/`for` body rebinding (`x = x + 1`) and `rec` fixpoint
feedback wires. It updates the nearest enclosing binding when the name is
already bound, or defines a new binding in the innermost scope when
unbound — mirroring `ScopeStack::assign` in the current VM.

`PushScope` pushes an empty scope without binding. Used by `eval_block`
to create a block-local scope before evaluating children.

### 2.7 Complete target ISA summary

| #   | Instruction                 | Category         | Stack effect                |
| --- | --------------------------- | ---------------- | --------------------------- |
| 1   | `Push(value)`               | Stack            | `-> val`                    |
| 2   | `Pop`                       | Stack            | `val ->`                    |
| 3   | `Dup`                       | Stack            | `val -> val val`            |
| 4   | `Swap`                      | Stack            | `a b -> b a`                |
| 5   | `Load(name)`                | Memory           | `-> val`                    |
| 6   | `Store(name)`               | Memory           | `val ->`                    |
| 7   | `LoadField(field)`          | Memory           | `obj -> val`                |
| 8   | `StoreField(field)`         | Memory           | `obj val -> obj`            |
| 9   | `Call(n)`                   | Control          | `callee arg.. -> result`    |
| 10  | `Return`                    | Control          | `val ->`                    |
| 11  | `Jump(target)`              | Control          | `->`                        |
| 12  | `Branch(target)`            | Control          | `bool ->`                   |
| 13  | `MakeClosure(params, body)` | Control          | `-> closure`                |
| 14  | `Arith(op)`                 | Hardware         | `a b -> result`             |
| 15  | `Ffi(module, name, n)`      | Hardware         | `arg.. -> result`           |
| 16  | `PatternMatch`              | Kernel primitive | `value pattern -> bindings` |
| 17  | `PopScope`                  | Scope stack      | `->`                        |
| 18  | `Assign(name)`              | Scope stack      | `val ->`                    |
| 19  | `PushScope`                 | Scope stack      | `->`                        |

**19 instructions total.** Of these, 2 (`Arith`, `Ffi`) are the hardware
boundary. The rest are the VM loop's instruction set. `PatternMatch` is a
kernel primitive that stays in Rust. Everything else above this is Handl
code compiled to bytecode.

---

## 3. Current ISA → Target ISA Mapping

The current ISA (`crates/plugin-engine-exec-bytecode/src/instr.rs`) has
~30 instructions, many of which are structural ops that should move to
the exec model (Handl code). This section maps each current instruction to
its target ISA decomposition.

### 3.1 Direct mappings (same concept)

| Current instruction                             | Target instruction          | Notes                                                                                                   |
| ----------------------------------------------- | --------------------------- | ------------------------------------------------------------------------------------------------------- |
| `Push(Value)`                                   | `Push(value)`               | Identical — push a literal.                                                                             |
| `Load(Rc<str>)`                                 | `Load(name)`                | Identical — scope stack lookup.                                                                         |
| `Store(Rc<str>)`                                | `Store(name)`               | Identical — push scope + bind.                                                                          |
| `PopScope`                                      | `PopScope`                  | Identical — pop scope.                                                                                  |
| `Call(usize)`                                   | `Call(n)`                   | Identical — apply callee to n args.                                                                     |
| `Jump(usize)`                                   | `Jump(target)`              | Identical — unconditional jump.                                                                         |
| `Return`                                        | `Return`                    | Identical — return top of stack.                                                                        |
| `Closure { params, rest, body, array_pattern }` | `MakeClosure(params, body)` | Same concept — capture env, create closure. The `rest`/`array_pattern` details become exec-model logic. |

### 3.2 Arithmetic mappings (consolidate to `Arith`)

The current ISA has 11 dedicated arithmetic bytecodes (`Add`, `Sub`,
`Mul`, `Div`, `Mod`, `Eq`, `Neq`, `Lt`, `Lte`, `Gt`, `Gte`) plus the
general `NativeOp(String, usize)` path. In the target ISA, all of these
consolidate into a single `Arith(op)` instruction.

| Current instruction  | Target instruction |
| -------------------- | ------------------ |
| `Add`                | `Arith("add")`     |
| `Sub`                | `Arith("sub")`     |
| `Mul`                | `Arith("mul")`     |
| `Div`                | `Arith("div")`     |
| `Mod`                | `Arith("mod")`     |
| `Eq`                 | `Arith("eq")`      |
| `Neq`                | `Arith("neq")`     |
| `Lt`                 | `Arith("lt")`      |
| `Lte`                | `Arith("lte")`     |
| `Gt`                 | `Arith("gt")`      |
| `Gte`                | `Arith("gte")`     |
| `NativeOp("add", 2)` | `Arith("add")`     |
| `NativeOp("sub", 2)` | `Arith("sub")`     |
| `NativeOp("mul", 2)` | `Arith("mul")`     |
| `NativeOp("div", 2)` | `Arith("div")`     |
| `NativeOp("mod", 2)` | `Arith("mod")`     |
| `NativeOp("neg", 1)` | `Arith("neg")`     |
| `NativeOp("eq", 2)`  | `Arith("eq")`      |
| `NativeOp("neq", 2)` | `Arith("neq")`     |
| `NativeOp("lt", 2)`  | `Arith("lt")`      |
| `NativeOp("gt", 2)`  | `Arith("gt")`      |
| `NativeOp("lte", 2)` | `Arith("lte")`     |
| `NativeOp("gte", 2)` | `Arith("gte")`     |
| `NativeOp("and", 2)` | `Arith("and")`     |
| `NativeOp("or", 2)`  | `Arith("or")`      |
| `NativeOp("not", 1)` | `Arith("not")`     |

The fixed-arity `Call1`/`Call2`/`Call3` variants are optimizations that
avoid per-call `Vec` allocation. In the target ISA, `Call(n)` is the
single instruction; the VM implementation can specialize internally for
small `n` without exposing it in the ISA.

| Current instruction | Target instruction |
| ------------------- | ------------------ |
| `Call1`             | `Call(1)`          |
| `Call2`             | `Call(2)`          |
| `Call3`             | `Call(3)`          |

### 3.3 Foreign op mapping

| Current instruction  | Target instruction     |
| -------------------- | ---------------------- |
| `ForeignOp(name, n)` | `Ffi("host", name, n)` |

`ForeignOp` calls a host function by name. In the target ISA, this is
`Ffi(module, name, n)` — the general host/OS syscall boundary. The
`module` field allows routing to different host registries (e.g.
`"host"`, `"io"`, `"fs"`, `"native"`).

### 3.4 Structural op mappings (decompose to target ISA)

These are the instructions that encode structural ops currently in Rust.
In the target ISA, they decompose into sequences of the minimal
instructions — they become Handl code in `exec_model.hnl` compiled to
bytecode.

#### `EvalChild(n)` → exec model (`proj_child`)

`EvalChild(n)` evaluates the n-th child of the current node. In the
target ISA, this is the `proj_child` structural op in `exec_model.hnl`:

```handl
export proj_child :: (node, role) ~> do {
  children :: getfield(node, "children")   ; LoadField("children")
  idx :: getfield(children, role)          ; LoadField(role)
  eval(idx)                                ; Call(eval)
}
```

Target bytecode sequence:

```
Load("node")              ; push the current node
LoadField("children")     ; get children array
Push(role)                ; push the role/index
LoadField(role)           ; get the child at role
Call(eval)                ; call eval on the child
```

| Current instruction | Target decomposition                                                            |
| ------------------- | ------------------------------------------------------------------------------- |
| `EvalChild(n)`      | `LoadField("children")`, `LoadField(n)`, `Call(eval)` (exec model `proj_child`) |
| `EvalChildOr(n)`    | Same as `EvalChild` + `Branch` for null check (exec model `proj_child_or`)      |

#### `LoadPayloadVar` / `LoadPayload` → exec model

| Current instruction | Target decomposition                                                                       |
| ------------------- | ------------------------------------------------------------------------------------------ |
| `LoadPayloadVar`    | `LoadField("payload")`, `Load` (exec model: read payload as string, look up that variable) |
| `LoadPayload`       | `LoadField("payload")` (exec model: push payload directly)                                 |

#### `StorePayloadName` → exec model

| Current instruction | Target decomposition                                                                                         |
| ------------------- | ------------------------------------------------------------------------------------------------------------ |
| `StorePayloadName`  | `LoadField("payload")`, `LoadField("name")`, `Store(name)` (exec model: read payload name, push scope, bind) |

#### `ChoiceJump(l, r)` → exec model (`Choice`)

`ChoiceJump` pops an Either value, pushes its carried value, and jumps to
the left or right target. In the target ISA, this is the `Choice` handler
in `arrow_model.hnl`:

```handl
"Choice": if is_left(incoming)
          then self(self, getfield(term, "l"), left_val(incoming), ctx)
          else self(self, getfield(term, "r"), right_val(incoming), ctx),
```

Target bytecode sequence:

```
; incoming is on the stack (an Either value)
Dup                      ; duplicate the Either
LoadField("__either__")  ; push "left" or "right"
Push("left")
Arith("eq")              ; compare to "left"
Branch(left_target)      ; jump to left branch if true
; fall through to right branch
LoadField("value")       ; push the right value
Jump(right_target)       ; jump to right branch

left_target:
LoadField("value")       ; push the left value
; ... left branch code ...
Jump(end)

right_target:
; ... right branch code ...

end:
```

| Current instruction | Target decomposition                                                                          |
| ------------------- | --------------------------------------------------------------------------------------------- |
| `ChoiceJump(l, r)`  | `LoadField("__either__")`, `Branch(l)`, `LoadField("value")`, `Jump(r)` (exec model `Choice`) |

#### `FirstOp(start, end)` → exec model (`First`)

`FirstOp` runs the inner sub-program on the first component of a pair,
then re-pairs with the second. In the target ISA, this is the `First`
handler in `arrow_model.hnl`:

```handl
"Fst": pair(self(self, getfield(term, "inner"), pair_fst(incoming), ctx),
            pair_snd(incoming)),
```

Target bytecode sequence:

```
; pair is on the stack: {"__pair__": [a, b]}
Dup                      ; duplicate the pair
LoadField("__pair__")    ; push [a, b]
Push(0)
LoadField(0)             ; push a (first component)
; ... inner sub-program on a ...
; result is on stack
Swap                     ; result, pair
LoadField("__pair__")   ; result, [a, b]
Push(1)
LoadField(1)             ; result, b (second component)
; construct new pair: {"__pair__": [result, b]}
; (via StoreField calls)
```

| Current instruction   | Target decomposition                                                                           |
| --------------------- | ---------------------------------------------------------------------------------------------- |
| `FirstOp(start, end)` | `LoadField("__pair__")`, index 0, `Call(inner)`, re-pair via `StoreField` (exec model `First`) |

#### `Pair` → exec model

`Pair` constructs `{"__pair__": [b, a]}` from two stack values. In the
target ISA, this is a sequence of `StoreField` calls (record construction
becomes Handl code):

```
; a (top), b (below) on stack
Push({})                 ; create empty record
Swap                     ; a, {}, b → rearrange
; ... StoreField calls to build {"__pair__": [b, a]} ...
```

| Current instruction | Target decomposition                                                                                   |
| ------------------- | ------------------------------------------------------------------------------------------------------ |
| `Pair`              | `Push({})`, `StoreField("__pair__")`, `StoreField` for array elements (exec model record construction) |

#### `LoopCheck(loop_back)` → exec model (`Loop`)

`LoopCheck` inspects a loop body result and routes single-step vs
multi-step. In the target ISA, this is the `Loop` handler in
`arrow_model.hnl`:

```handl
"Loop": trace(self, getfield(term, "body"), incoming, ctx),
```

Target bytecode sequence:

```
; body result is on the stack
Dup                      ; duplicate the result
LoadField("__either__")  ; check if it's an Either
Push(null)
Arith("neq")             ; is it an Either?
Branch(is_either)

; not an Either — it's a pair (single-step mode)
LoadField("__pair__")    ; push [out, _state]
Push(0)
LoadField(0)             ; push out
Jump(done)               ; fall through

is_either:
Dup                      ; duplicate the Either
LoadField("__either__")  ; push "left" or "right"
Push("left")
Arith("eq")              ; is it a left?
Branch(is_left)

; right — continue loop
LoadField("value")       ; push new state
; reconstruct pair (original_input, new_state)
; ... StoreField calls ...
Jump(loop_back)          ; jump to loop body start

is_left:
LoadField("value")       ; push the final value
Jump(done)

done:
```

| Current instruction    | Target decomposition                                                                             |
| ---------------------- | ------------------------------------------------------------------------------------------------ |
| `LoopCheck(loop_back)` | `LoadField("__either__")`, `Branch`, `LoadField("value")`, `Jump(loop_back)` (exec model `Loop`) |

#### `DerivedStepOp(op, ranges)` → exec model

`DerivedStepOp` re-evaluates arg sub-programs per iteration. In the
target ISA, this becomes Handl code in `arrow_model.hnl` that re-runs the
sub-schemes with the current value as incoming, then dispatches to the
native op via `call_op` (which falls back to `Ffi`).

| Current instruction         | Target decomposition                                                        |
| --------------------------- | --------------------------------------------------------------------------- |
| `DerivedStepOp(op, ranges)` | Re-eval args via `Call`, then `Ffi("native", op, n)` (exec model loop step) |

#### `RouteOnType(tag)` → exec model

`RouteOnType` compares a value's type tag and produces an Either. In the
target ISA, this is `LoadField("__tag__")` + `Arith("eq")` + `Branch`:

```
; value on stack
Dup
LoadField("__tag__")      ; push the type tag
Push(tag)
Arith("eq")               ; compare to expected tag
Branch(matches)           ; if match, go to left branch
; no match — push either_right(value)
Jump(end)
matches:
; match — push either_left(value)
end:
```

| Current instruction | Target decomposition                                                           |
| ------------------- | ------------------------------------------------------------------------------ |
| `RouteOnType(tag)`  | `LoadField("__tag__")`, `Arith("eq")`, `Branch` (exec model type-tag dispatch) |

#### `EvalNode(idx)` → exec model

`EvalNode` evaluates an AST node at an absolute arena index. In the
target ISA, this is `Load(node)` + `Call(eval)` — the exec model's `eval`
function handles node evaluation.

| Current instruction | Target decomposition                                    |
| ------------------- | ------------------------------------------------------- |
| `EvalNode(idx)`     | `Load(node)`, `Call(eval)` (exec model node evaluation) |

#### `MakeObj { keys }` → exec model (record construction)

`MakeObj` pops `keys.len()` values and constructs an object. In the
target ISA, this is a sequence of `StoreField` calls — the `record`
structural op in `exec_model.hnl`:

```handl
export record :: (fields) ~> do {
  result :: {}
  foreach pair in fields {
    setfield(result, getfield(pair, 0), getfield(pair, 1))
  }
  result
}
```

Target bytecode sequence:

```
Push({})                  ; create empty record
; for each key/value pair:
Swap                      ; arrange value, record
StoreField(key)           ; set key to value
; repeat for all keys
```

| Current instruction | Target decomposition                                             |
| ------------------- | ---------------------------------------------------------------- |
| `MakeObj { keys }`  | `Push({})`, `StoreField(key)` for each key (exec model `record`) |

#### `NativeOp(op, n)` — general native ops

General `NativeOp` dispatches to a named native op. In the target ISA,
arithmetic ops map to `Arith(op)`, and all other native ops map to
`Ffi("native", op, n)` — the host function registry fallback. The
`call_op` structural op in `exec_model.hnl` first searches the handler
stack, then falls back to `Ffi("native", name, args)`.

| Current instruction            | Target decomposition                               |
| ------------------------------ | -------------------------------------------------- |
| `NativeOp("add"/"sub"/..., n)` | `Arith(op)`                                        |
| `NativeOp("tag", 1)`           | `LoadField("__tag__")` (exec model `typeof`)       |
| `NativeOp("id", 0)`            | no-op (the value is already on the stack)          |
| `NativeOp("closure", n)`       | `MakeClosure(params, body)` (exec model `closure`) |
| `NativeOp("bind", 2)`          | `Store(name)` (exec model `extend_env`)            |
| `NativeOp("loop_step", n)`     | exec model loop logic (Handl code)                 |
| `NativeOp(op, n)` (other)      | `Ffi("native", op, n)` (host fallback)             |

### 3.5 Summary: current → target mapping

| Current instruction            | Target                              | Category               |
| ------------------------------ | ----------------------------------- | ---------------------- |
| `Push`                         | `Push`                              | Direct                 |
| `Load`                         | `Load`                              | Direct                 |
| `LoadPayloadVar`               | `LoadField` + `Load`                | Decompose (exec model) |
| `LoadPayload`                  | `LoadField`                         | Decompose (exec model) |
| `EvalChild`                    | `LoadField` + `Call`                | Decompose (exec model) |
| `EvalChildOr`                  | `LoadField` + `Branch` + `Call`     | Decompose (exec model) |
| `Store`                        | `Store`                             | Direct                 |
| `StorePayloadName`             | `LoadField` + `Store`               | Decompose (exec model) |
| `PopScope`                     | `PopScope`                          | Direct                 |
| `Call`/`Call1`/`Call2`/`Call3` | `Call(n)`                           | Direct (consolidate)   |
| `ChoiceJump`                   | `LoadField` + `Branch` + `Jump`     | Decompose (exec model) |
| `Jump`                         | `Jump`                              | Direct                 |
| `NativeOp` (arith)             | `Arith(op)`                         | Consolidate            |
| `NativeOp` (other)             | `Ffi("native", op, n)`              | Consolidate            |
| `ForeignOp`                    | `Ffi(module, name, n)`              | Direct (rename)        |
| `FirstOp`                      | `LoadField` + `Call` + `StoreField` | Decompose (exec model) |
| `Pair`                         | `Push({})` + `StoreField`           | Decompose (exec model) |
| `LoopCheck`                    | `LoadField` + `Branch` + `Jump`     | Decompose (exec model) |
| `DerivedStepOp`                | `Call` + `Ffi`                      | Decompose (exec model) |
| `Return`                       | `Return`                            | Direct                 |
| `RouteOnType`                  | `LoadField` + `Arith` + `Branch`    | Decompose (exec model) |
| `Add`..`Gte`                   | `Arith(op)`                         | Consolidate            |
| `EvalNode`                     | `Load` + `Call`                     | Decompose (exec model) |
| `MakeObj`                      | `Push({})` + `StoreField`           | Decompose (exec model) |
| `Closure`                      | `MakeClosure`                       | Direct (rename)        |

---

## 4. The Hardware Boundary

### 4.1 The three categories

The hardware boundary is the irreducible set of operations that touch the
actual machine. Everything above it is bytecode or Handl. There are
exactly three categories:

#### 1. Arithmetic (ALU)

CPU arithmetic and logic operations. These map directly to hardware ALU
instructions:

| Operation | Arity  | Description           |
| --------- | ------ | --------------------- |
| `add`     | binary | Addition              |
| `sub`     | binary | Subtraction           |
| `mul`     | binary | Multiplication        |
| `div`     | binary | Division              |
| `mod`     | binary | Modulo                |
| `neg`     | unary  | Negation              |
| `eq`      | binary | Equality              |
| `neq`     | binary | Inequality            |
| `lt`      | binary | Less than             |
| `gt`      | binary | Greater than          |
| `lte`     | binary | Less than or equal    |
| `gte`     | binary | Greater than or equal |
| `and`     | binary | Logical AND           |
| `or`      | binary | Logical OR            |
| `not`     | unary  | Logical NOT           |

These are exposed via the single `Arith(op)` instruction. The VM
implementation dispatches on `op` to the corresponding Rust arithmetic
operation. No Handl code is needed — these are the raw ALU.

> **Tagged arithmetic is exec model.** `number_add` (untag, add, re-tag)
> is exec-model semantics in `exec_model.hnl`, not a kernel primitive.
> The kernel only provides raw `add`; the tagging/untagging is Handl
> orchestration over `LoadField`/`StoreField`/`Arith`.

#### 2. Memory (load/store)

Memory access operations. In the VM, these are:

| Operation      | Instruction         | Description                            |
| -------------- | ------------------- | -------------------------------------- |
| Variable load  | `Load(name)`        | Look up a name in the scope stack      |
| Variable store | `Store(name)`       | Push scope + bind name to value        |
| Field load     | `LoadField(field)`  | Get a field from a record on the stack |
| Field store    | `StoreField(field)` | Set a field on a record on the stack   |
| Scope push     | `PushScope`         | Push a new empty scope                 |
| Scope pop      | `PopScope`          | Pop the innermost scope                |
| Assign         | `Assign(name)`      | Update-or-define a binding             |

The scope stack is a VM-managed data structure (see §7). `Load`/`Store`
operate on it. `LoadField`/`StoreField` operate on record values on the
operand stack — these are the `getfield`/`setfield` primitives.

> **`typeof` → `getfield "__tag__"`.** Type inspection is not a kernel
> primitive. It's `LoadField("__tag__")` — a field access on a tagged
> record. The exec model's `typeof_tag` function is Handl code that calls
> `LoadField`.

> **`alloc`/`drop` → `ffi` to allocator, or `mmap` + Handl GC.** Memory
> allocation is not a kernel primitive. It's `Ffi("alloc", "alloc", n)`
> or `Ffi("mem", "mmap", n)` + Handl GC logic. The kernel only provides
> the `ffi` boundary to reach the allocator.

#### 3. FFI (host/OS syscall boundary)

`Ffi(module, name, n)` is the **only** way to reach effects — I/O,
syscalls, network, files, time, randomness. Without `Ffi`, the VM is
pure computation over memory: it can load, store, compute, and branch,
but it cannot observe or affect the outside world.

The host runtime maintains a registry of `(module, name)` → Rust
function mappings. When the VM encounters `Ffi(module, name, n)`, it:

1. Pops `n` argument values from the operand stack.
2. Looks up `module::name` in the host function registry.
3. Calls the Rust function with the arguments.
4. Pushes the result onto the operand stack.

If the function is not found, the VM raises an error.

### 4.2 What is NOT in the hardware boundary

The following are **not** hardware boundary operations. They are Handl
code compiled to bytecode:

| Concept                          | How it's expressed                                          |
| -------------------------------- | ----------------------------------------------------------- |
| `typeof`                         | `LoadField("__tag__")`                                      |
| `alloc`/`drop`                   | `Ffi("alloc", ...)` or `Ffi("mem", "mmap", ...)` + Handl GC |
| `getfield`/`setfield`            | `LoadField`/`StoreField` (VM instructions)                  |
| `call`/`ret`                     | `Call`/`Return` (VM instructions)                           |
| `jump`/`branch`                  | `Jump`/`Branch` (VM instructions)                           |
| `make_closure`                   | `MakeClosure` (VM instruction)                              |
| `pattern_match`                  | `PatternMatch` (kernel primitive, stays in Rust)            |
| `proj_child`                     | exec model Handl code (`LoadField` + `Call`)                |
| `eval_block`                     | exec model Handl code (`PushScope` + `Call` + `PopScope`)   |
| `push_handler`/`pop_handler`     | exec model Handl code (handler stack management)            |
| `apply`                          | exec model Handl code (`Call`)                              |
| `record`/`array`/`tag_construct` | exec model Handl code (`StoreField` sequences)              |
| `call_op`                        | exec model Handl code (handler search + `Ffi` fallback)     |
| `import`/`export`                | exec model Handl code (`Ffi("module", ...)` + `Store`)      |

### 4.3 The effect boundary

`Ffi` is the only way to reach effects. This has a profound property:
**without `Ffi`, the VM is a pure computation over memory.** A bytecode
program that contains no `Ffi` instructions is referentially transparent
— it can be paused, serialized, resumed, replayed, and reasoned about
purely. Effects (I/O, syscalls, network, files, time, randomness) are
quarantined behind `Ffi`.

This is the foundation for the serialization boundary (see §5).

---

## 5. The Serialization Boundary

### 5.1 The boundary

ADR-0103 defines an exact serialization boundary:

| Layer                           | Serializable? | Resumable?       |
| ------------------------------- | ------------- | ---------------- |
| Handl source                    | Yes           | N/A (recompiled) |
| Bytecode                        | Yes           | Yes              |
| Terms (tagged records)          | Yes           | Yes              |
| Eval schemes                    | Yes           | Yes              |
| Models                          | Yes           | Yes              |
| Environment state (scope stack) | Yes           | Yes              |
| Values                          | Yes           | Yes              |
| VM loop (Rust)                  | No            | No               |
| Hardware boundary (Rust)        | No            | No               |
| `Ffi` call state                | No            | No               |

The VM loop and hardware boundary are Rust — opaque, not serializable, not
resumable. This is the fixed substrate by design. Everything above it is
serializable data.

### 5.2 Native op implementations vs. native op calls

The distinction is precise:

- **Native op implementations** (the VM loop + hardware boundary) are
  Rust — opaque, not serializable, not resumable. This is the fixed
  substrate.
- **Native op calls** (bytecode instructions like `Arith`, `Ffi`,
  `Call`, `Load`) are serializable as bytecode. A bytecode program is a
  `Vec<Instr>` — serializable data.

### 5.3 Resumability rules

- **Computation that stays in Handl data/bytecode** can be paused,
  serialized, and resumed. The VM state (instruction pointer, operand
  stack, scope stack, handler stack) is serializable. A paused
  computation can be resumed on a different machine, at a different time,
  or replayed deterministically.

- **Computation that crosses into `Ffi`** cannot be resumed. When the VM
  executes an `Ffi` instruction, control passes to a Rust function that
  may hold OS-level state (file handles, network sockets, etc.). This
  state is not serializable. If the VM is paused during an `Ffi` call, the
  computation cannot be resumed — the native state is lost.

- **Computation that calls `PatternMatch`** can be resumed. The
  pattern-matching logic is pure (it binds names in the scope stack and
  compares values). It does not touch the OS. A `PatternMatch` call is
  serializable.

### 5.4 Implications for effect tracking

Because `Ffi` is the only effect boundary, effect tracking reduces to
**scanning bytecode for `Ffi` instructions**. A function that contains no
`Ffi` instructions is pure. A function that contains `Ffi` instructions
has effects. The `module` and `name` fields of each `Ffi` instruction
identify the specific effect (e.g. `Ffi("io", "print", 1)` has an I/O
effect).

This is the foundation for capability-based effect tracking: the host
runtime can grant or deny `Ffi` access to specific `(module, name)` pairs,
giving fine-grained control over what effects a bytecode program can
perform.

---

## 6. Migration Plan

### 6.1 The key insight

The migration from the current ISA (~30 instructions, many structural) to
the target minimal ISA (~19 instructions, only irreducible) follows a
single principle: **structural ops become Handl functions in
`exec_model.hnl` that compile to the minimal ISA.**

The current ISA encodes structural ops as Rust instructions (`EvalChild`,
`ChoiceJump`, `FirstOp`, `LoopCheck`, `MakeObj`, etc.). The target ISA
removes these instructions and replaces them with Handl code that
orchestrates the minimal instructions (`LoadField`, `StoreField`, `Call`,
`Branch`, `Jump`, etc.).

### 6.2 Migration phases

#### Phase 1a: Implement the target VM loop

Implement the new VM loop in Rust that executes the 19 target
instructions. This is a new crate (or a new module in the existing
bytecode crate) that contains:

- The `Instr` enum with 19 variants (the target ISA).
- The fetch-decode-execute loop.
- The scope stack (`ScopeStack` — reused from the current VM).
- The handler stack (new — a `Vec<Value>` of handler values).
- The `Arith` dispatch (match on `op` to Rust arithmetic).
- The `Ffi` dispatch (look up `module::name` in the host registry).
- The `PatternMatch` primitive (reuse `pattern_match.rs`).
- Auto-pop of scopes and handlers on failure.

This phase does not remove any existing instructions. It adds the new
VM alongside the existing one.

#### Phase 1b: Implement the Handl → target bytecode compiler

Implement a compiler that compiles Handl source (`.hnl` files) to the
target ISA. This compiler initially lives in Rust (later it will be
reauthored in Handl per the bootstrap stages in ADR-0103).

The compiler must handle:

- Variable references → `Load(name)`
- Bindings (`x = expr`) → compile expr, `Store(name)` or `Assign(name)`
- Function calls → compile callee + args, `Call(n)`
- Closures (`~>`) → `MakeClosure(params, body)`
- Field access (`obj.field`) → `LoadField(field)`
- Record construction (`{key: val, ...}`) → `Push({})`, `StoreField(key)` per field
- Conditionals (`if/else`) → `Branch` + `Jump`
- `match` → `PatternMatch` + arm iteration
- `while`/`for` loops → `Jump`/`Branch` loop patterns
- `do { ... }` blocks → `PushScope` + body + `PopScope`

#### Phase 1c: Author `exec_model.hnl` structural ops as Handl code

The structural ops in `exec_model.hnl` (already drafted — see
`exec_model.hnl`) are compiled to the target ISA. Each structural op
becomes a Handl function:

| Structural op            | Target bytecode                                          |
| ------------------------ | -------------------------------------------------------- |
| `proj_child(node, role)` | `LoadField("children")`, `LoadField(role)`, `Call(eval)` |
| `extend_env(name, val)`  | `PushScope`, `Store(name)`                               |
| `eval_block(node)`       | `PushScope`, `Call(eval)` per child, `PopScope`          |
| `push_handler(handler)`  | `LoadField(ctx, "handler_stack")`, `StoreField`          |
| `apply(callee, args)`    | `Call(n)`                                                |
| `record(fields)`         | `Push({})`, `StoreField` per field                       |
| `call_op(name, args)`    | handler search loop, `Ffi("native", name, n)` fallback   |

#### Phase 1d: Author `arrow_model.hnl` handlers as Handl code

The arrow-calculus handlers in `arrow_model.hnl` (already drafted) are
compiled to the target ISA. Each handler becomes a Handl function:

| Handler  | Target bytecode                                  |
| -------- | ------------------------------------------------ |
| `Arr`    | `Push(value)`                                    |
| `Seq`    | `Call(fst)`, `Call(snd)`                         |
| `First`  | `LoadField("__pair__")`, `Call(inner)`, re-pair  |
| `Choice` | `LoadField("__either__")`, `Branch`, `Call(l/r)` |
| `Loop`   | `Jump`/`Branch` loop pattern                     |
| `App`    | `Call(callee)`, `Call(n)`                        |
| `Handle` | `push_handler`, `Call(body)`, `pop_handler`      |

#### Phase 2: Make eval schemes Handl-callable

Convert `eval.yaml` schemes from `Term` values to Handl functions calling
structural ops. An eval scheme for `assign` becomes:

```handl
; eval scheme for assign:
val :: proj_child("value")       ; LoadField("children"), LoadField("value"), Call(eval)
extend_env(proj_child("target"), val)  ; LoadField("children"), LoadField("target"), Call(eval), PushScope, Store
val                              ; Load("val")
```

These compile to the target ISA. The VM runs the compiled bytecode.

#### Phase 3: Verify parity

Run the existing test suite (`exec_model_parity.rs`,
`option_b_frontier_e2e.rs`, etc.) against the new VM. The new VM must
produce results identical to the current tree-walk and bytecode engines.

#### Phase 4: Remove old instructions

Once all structural ops are Handl code and all tests pass, remove the old
instructions from the ISA:

- Remove `EvalChild`, `EvalChildOr` → replaced by `proj_child` Handl code
- Remove `LoadPayloadVar`, `LoadPayload` → replaced by exec model code
- Remove `StorePayloadName` → replaced by exec model code
- Remove `ChoiceJump` → replaced by `Choice` Handl code
- Remove `FirstOp` → replaced by `First` Handl code
- Remove `Pair` → replaced by record construction Handl code
- Remove `LoopCheck` → replaced by `Loop` Handl code
- Remove `DerivedStepOp` → replaced by loop step Handl code
- Remove `RouteOnType` → replaced by type-tag dispatch Handl code
- Remove `EvalNode` → replaced by `Load` + `Call(eval)`
- Remove `MakeObj` → replaced by `record` Handl code
- Remove `NativeOp` → replaced by `Arith` + `Ffi`
- Remove `Call1`/`Call2`/`Call3` → consolidated into `Call(n)`
- Remove `Add`/`Sub`/`Mul`/`Div`/`Mod`/`Eq`/`Neq`/`Lt`/`Lte`/`Gt`/`Gte` → consolidated into `Arith(op)`

After this phase, the ISA is the 19-instruction target ISA.

#### Phase 5: Remove the `Term` enum

Replace the 28-variant `Term` enum with `serde_json::Value` (tagged
records). Update all constructors and pattern matches. The VM no longer
knows what any tag means — it just executes bytecode.

### 6.3 Migration ordering

The migration is designed to be **incremental and reversible**. Each
phase produces a working system:

1. **Phase 1a** (new VM loop) — the old VM still works; the new VM is
   added alongside.
2. **Phase 1b** (compiler) — the compiler targets the new ISA; old
   bytecode still runs on the old VM.
3. **Phase 1c** (exec model) — structural ops are Handl code; the old
   instructions still exist as fallback.
4. **Phase 1d** (arrow model) — arrow handlers are Handl code.
5. **Phase 2** (eval schemes) — eval schemes are Handl functions.
6. **Phase 3** (parity) — verify both VMs produce identical results.
7. **Phase 4** (remove old instructions) — only after parity is verified.
8. **Phase 5** (remove `Term` enum) — final cleanup.

At any point before Phase 4, the system can fall back to the old VM and
old instructions. Phase 4 is the point of no return — but it only happens
after parity is verified.

---

## 7. VM-Managed Data Structures

### 7.1 The operand stack

A `Vec<Value>` — the main computation stack. Instructions push and pop
values. This is the standard stack-machine operand stack.

```
operand_stack: Vec<Value>
```

### 7.2 The scope stack

The scope stack is a VM-managed data structure for variable bindings. It
is a flat list of `(name, value)` bindings with scope markers, searched
innermost-first. This is reused from the current VM
(`crates/plugin-engine-exec-bytecode/src/vm.rs`, `ScopeStack`).

```
ScopeStack {
    bindings: Vec<(Rc<str>, TaggedValue)>,  // flat list, innermost last
    marks: Vec<usize>,                       // scope markers
    captured_root: Option<Rc<FxHashMap<Rc<str>, TaggedValue>>>,  // closure env
    globals: Rc<FxHashMap<Rc<str>, TaggedValue>>,  // host globals
    enclosing_updated: bool,                 // fixpoint detection
}
```

#### Operations

| Operation           | Instruction    | Description                                                              |
| ------------------- | -------------- | ------------------------------------------------------------------------ |
| `push_scope()`      | `PushScope`    | Record current `bindings.len()` as a mark.                               |
| `pop_scope()`       | `PopScope`     | Truncate `bindings` back to the last mark.                               |
| `define(name, val)` | `Store(name)`  | Push `(name, val)` onto `bindings`.                                      |
| `lookup(name)`      | `Load(name)`   | Reverse linear scan of `bindings`, then `captured_root`, then `globals`. |
| `assign(name, val)` | `Assign(name)` | Update nearest binding, or define in innermost scope.                    |

#### Lookup ordering

Variable lookup searches in this order (matching the current VM):

1. **Innermost bindings** (reverse linear scan of `bindings`).
2. **Captured root** (the closure's captured environment).
3. **Host globals** (e.g. `self`, `__arg_N`).

User-defined scopes shadow captured variables, which shadow globals.

#### Closure capture

When `MakeClosure` executes, the VM calls `scopes.capture()` to snapshot
the current bindings as an `Rc<FxHashMap>`. When the closure is later
applied via `Call`, the VM:

1. Saves the current scope stack.
2. Sets `captured_root` to the closure's captured environment.
3. Pushes a new scope.
4. Binds the parameters.
5. Evaluates the body.
6. Restores the original scope stack.

#### Fixpoint detection (`enclosing_updated`)

The `enclosing_updated` flag is set when `Assign(name)` updates a binding
in an _enclosing_ scope (below the innermost mark) with a different value.
This is read and cleared by the `eval_rec` fixpoint logic — such writes
are the ADR-0074 feedback wires (`while`/`do-while`/`for` body rebinding).
A pass that produces any such write must not count as converged.

### 7.3 The handler stack

The handler stack is a VM-managed data structure for algebraic-effect
handler dispatch. It is a list of handler values. The `call_op` structural
op searches it (innermost-first) to find a handler providing an op.

```
handler_stack: Vec<Value>  // list of handler values
```

Each handler value is a record with an `impls` field — a record mapping
op names to handler implementations:

```json
{
  "impls": { "read": <arrow>, "write": <arrow> },
  "resume": <arrow>,
  ...
}
```

#### Operations

| Operation               | Description                                                                                            |
| ----------------------- | ------------------------------------------------------------------------------------------------------ |
| `push_handler(handler)` | Push `handler` onto `handler_stack`.                                                                   |
| `pop_handler()`         | Pop the top handler from `handler_stack`.                                                              |
| `find_op_handler(name)` | Walk `handler_stack` innermost-first; return the first handler whose `impls` has a field named `name`. |

`push_handler` and `pop_handler` are structural ops in `exec_model.hnl`.
They manage the handler stack via `LoadField`/`StoreField` on the context
record. The `call_op` structural op searches the handler stack and falls
back to `Ffi("native", name, n)` when no handler intercepts.

### 7.4 Auto-pop on failure (ADR-0103 resolved decision #6)

The VM **auto-pops scopes and handlers on failure**. No `try/finally`
primitive is needed in the metalanguage.

When the VM encounters an error (e.g. a failed `Ffi` call, a type error in
`Arith`, a failed `PatternMatch` that the exec model doesn't catch), the
VM:

1. Pops all scopes pushed since the last `Call` or `PushScope` that
   established a recovery point.
2. Pops all handlers pushed since the last `Call` or `push_handler` that
   established a recovery point.
3. Propagates the error to the caller.

This means structural ops in `exec_model.hnl` do not need explicit
cleanup in error paths. `eval_block` pushes a scope, evaluates children,
and pops the scope on success — but if a child fails, the VM auto-pops
the scope. Similarly, `push_handler`/`pop_handler` only need explicit
`pop_handler` on the success path; the VM auto-pops on failure.

#### Implementation

The VM tracks a **cleanup stack** — a stack of `(scope_mark, handler_mark)`
pairs. Each `PushScope` or `push_handler` pushes a cleanup entry. Each
`PopScope` or `pop_handler` pops the corresponding entry. On failure, the
VM unwinds the cleanup stack, popping scopes and handlers back to the
nearest call boundary.

```
cleanup_stack: Vec<(scope_mark: usize, handler_mark: usize)>
```

When `Call` is executed, the VM saves the current cleanup stack depth.
If the callee fails, the VM unwinds to that depth, popping all scopes
and handlers that were pushed during the call.

### 7.5 The call frame stack

The VM maintains a call frame stack for `Call`/`Return`:

```
CallFrame {
    instrs: Rc<[Instr]>,     // the bytecode being executed
    ip: usize,               // instruction pointer
    operand_stack_base: usize,  // operand stack depth at call entry
    scope_mark: usize,        // scope stack mark at call entry
    handler_mark: usize,      // handler stack mark at call entry
    cleanup_depth: usize,     // cleanup stack depth at call entry
}
```

`Call(n)` pushes a new call frame. `Return` pops the current frame and
restores the caller's state. On failure, the VM unwinds call frames,
popping scopes and handlers as it goes (auto-pop).

---

## 8. Value Representation

The VM operates on the same tagged JSON value shapes as the current
tree-walk and bytecode engines:

| Shape               | Representation                                                      | Used by                                                                 |
| ------------------- | ------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| **Pair**            | `{"__pair__": [a, b]}`                                              | `First` combinator, loop state                                          |
| **Either**          | `{"__either__": "left"\|"right", "value": v}`                       | `Choice` combinator, loop routing                                       |
| **Arrow** (closure) | `{"__arrow__": true, "params": [...], "body": <idx>, "env": {...}}` | `Call`, `MakeClosure`                                                   |
| **Record** (object) | `{"key1": val1, "key2": val2, ...}`                                 | `LoadField`/`StoreField`, record construction                           |
| **Array**           | `[val1, val2, ...]`                                                 | Array operations                                                        |
| **Number**          | `{"__tag__": "Number", "value": <num>}`                             | Tagged arithmetic (exec model)                                          |
| **Tagged**          | `{"__tag__": <tag>, "value": <payload>}`                            | Universal data representation (ADR-0103: `Term` is `serde_json::Value`) |

`typeof` is `LoadField("__tag__")` — a field access on a tagged record.
The exec model's `typeof_tag` function is Handl code that calls
`LoadField`.

---

## 9. Bytecode Example: `assign` eval scheme

To illustrate how the target ISA works, here is the bytecode for the
`assign` eval scheme. The eval scheme for `assign` is:

```handl
val :: proj_child("value")           ; evaluate the value child
extend_env(proj_child("target"), val) ; bind the target name to the value
val                                   ; return the value
```

Compiled to target bytecode:

```bytecode
; proj_child("value"):
Load("node")              ; push the current node
LoadField("children")     ; push node.children
Push("value")             ; push the role name "value"
LoadField("value")        ; push children["value"] — the value child node
Call(eval)                ; call eval(child_node) — result on stack
Store("val")              ; bind "val" in a new scope

; proj_child("target"):
Load("node")              ; push the current node
LoadField("children")     ; push node.children
Push("target")            ; push the role name "target"
LoadField("target")       ; push children["target"] — the target child node
Call(eval)                ; call eval(child_node) — result is the target name

; extend_env(target_name, val):
Swap                      ; arrange: target_name, val
Store("target")           ; bind "target" in a new scope (or use Assign for update-or-define)

; val:
Load("val")               ; push the value
Return                    ; return it
```

The VM executes this bytecode. It does not know what `assign` means — it
just runs the instructions. The semantics come from the eval scheme
(Handl code), not from the kernel.

---

## 10. Bytecode Example: `if`/`else`

The `if`/`else` construct compiles to `Branch` + `Jump`:

```handl
if cond { then_branch } else { else_branch }
```

Compiled to target bytecode:

```bytecode
; evaluate condition
; ... compile cond expression ...
; bool is on stack

Branch(then_target)       ; pop bool, jump to then if true
; fall through to else
; ... compile else_branch ...
Jump(end)                 ; skip then branch

then_target:
; ... compile then_branch ...

end:
; result is on stack
```

---

## 11. Bytecode Example: `match`

The `match` construct uses `PatternMatch` for each arm:

```handl
match value {
  Pattern1 => body1
  Pattern2 => body2
  _ => default
}
```

Compiled to target bytecode:

```bytecode
; value is on stack

; arm 1
Dup                      ; duplicate value for pattern matching
Push(pattern1)           ; push pattern1 AST
PatternMatch             ; match value against pattern1
Branch(arm1_body)        ; if match succeeded (non-null), jump to arm1 body
Pop                      ; discard the failed match result (null)

; arm 2
Dup                      ; duplicate value
Push(pattern2)           ; push pattern2 AST
PatternMatch             ; match value against pattern2
Branch(arm2_body)        ; if match succeeded, jump to arm2 body
Pop                      ; discard the failed match result

; default arm
Jump(default_body)       ; jump to default

arm1_body:
; bindings from PatternMatch are in the scope
; ... compile body1 ...
Jump(end)

arm2_body:
; ... compile body2 ...
Jump(end)

default_body:
; ... compile default ...
Jump(end)

end:
; result is on stack
```

The `match` op is exec model — Handl code that orchestrates arm iteration
using the `PatternMatch` kernel primitive. The kernel provides
`PatternMatch`; the exec model provides the arm iteration logic.

---

## 12. Open Design Questions

### 12.1 `MakeClosure` body representation

`MakeClosure(params, body)` needs to reference the closure body. Options:

- **(a)** `body` is an instruction-range `(start, end)` into the current
  bytecode program (like the current `FirstOp`).
- **(b)** `body` is an AST node index (like the current `Closure`
  instruction), and the VM compiles it on first call (with caching).
- **(c)** `body` is a separate bytecode program (a `Rc<[Instr]>`).

Option (a) is the most consistent with the bytecode-VM vision — closures
are bytecode ranges, not AST nodes. Option (b) requires the VM to know
about AST compilation, which is a leaky abstraction. Option (c) is clean
but requires allocating separate bytecode arrays.

**Recommendation: (a)** — instruction ranges. The compiler lays out
closure bodies as bytecode ranges within the program. `MakeClosure`
captures `(start, end)` and the current environment. When applied, the
VM executes the body range with the captured environment + parameter
bindings.

### 12.2 `PatternMatch` pattern representation

`PatternMatch` takes a pattern and a value. The pattern is currently an
AST node (`AstRef`). In the target ISA, should the pattern be:

- **(a)** An AST node reference (the VM needs AST access).
- **(b)** A pre-compiled pattern descriptor (a `Value` describing the
  pattern shape).

Option (b) is cleaner — the VM should not need AST access for
pattern matching. But it requires a pattern- compilation step. For the
initial implementation, option (a) is simpler (reuse the existing
`pattern_match.rs`).

**Recommendation: (a) for initial implementation, (b) as future work.**

### 12.3 Handler stack location

The handler stack is currently described as living in a context record
(`ctx.handler_stack`), managed via `LoadField`/`StoreField`. An
alternative is to make the handler stack a VM-managed data structure
(like the scope stack), with dedicated instructions `PushHandler`/
`PopHandler`.

**Recommendation:** Start with the context-record approach (matching
`exec_model.hnl`), since it keeps the ISA minimal. If performance
demands it, add `PushHandler`/`PopHandler` as VM instructions later.

### 12.4 `Arith` on tagged values

`Arith(op)` operates on raw values. But the current system uses tagged
numbers (`{"__tag__": "Number", "value": <num>}`). Should `Arith`:

- **(a)** Operate on raw values only (exec model handles tagging).
- **(b)** Handle tagged values directly (faster, but leaks semantics into
  the kernel).

Per ADR-0103, `number_add` is exec-model semantics. The kernel only
provides raw `add`. **Recommendation: (a)** — `Arith` operates on raw
values. The exec model's `number_add` untags, calls `Arith("add")`, and
re-tags.

---

## 13. Summary

The ADR-0103 VM is a minimal bytecode machine with 19 instructions. The
kernel is the VM loop + hardware boundary (`Arith`, `Ffi`) + the
`PatternMatch` primitive. Everything else — structural ops, eval schemes,
arrow-calculus combinators, type judgments — is Handl code compiled to
bytecode and run on the VM.

The migration from the current ~30-instruction ISA to the 19-instruction
target ISA is incremental and reversible: structural ops become Handl
functions, old instructions are removed only after parity is verified.

The hardware boundary is exact: `Arith` (ALU), `Ffi` (OS/effects), and
memory operations (`Load`/`Store`/`LoadField`/`StoreField`). Without
`Ffi`, the VM is pure computation over memory. `Ffi` is the only way to
reach effects — I/O, syscalls, network, files, time, randomness.

The serialization boundary is exact: the VM loop + hardware boundary are
Rust (opaque, not serializable); everything above is serializable data.
Computation that stays in Handl data/bytecode can be paused, serialized,
and resumed. Computation that crosses into `Ffi` cannot be resumed.

The VM manages two key data structures: the scope stack (variable
bindings with scope markers) and the handler stack (algebraic-effect
handler dispatch). The VM auto-pops scopes and handlers on failure — no
`try/finally` primitive is needed in the metalanguage.
