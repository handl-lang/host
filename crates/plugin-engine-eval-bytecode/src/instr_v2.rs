//! V2 bytecode instruction set for the arrow-calculus kernel (ADR-0052).
//!
//! Each [`SchemeV2`](handl_language::scheme_v2::SchemeV2) combinator compiles
//! to a flat sequence of [`InstrV2`] values. The v2 VM executes them against an
//! operand stack and a scope stack, producing values identical to the v2
//! tree-walk reference handler.
//!
//! This is **additive**: the v1 [`crate::Instr`] enum and its compiler/VM are
//! untouched. The v2 ISA is a separate instruction set for the six
//! arrow-calculus primitives.
//!
//! ## ISA overview
//!
//! | Instruction          | Stack effect                          | Description |
//!|----------------------|---------------------------------------|-------------|
//! | [`Push`]             | `-> val`                              | Push a literal value. |
//! | [`Load`]             | `-> val`                              | Look up a named variable in the scope stack. |
//! | [`LoadPayloadVar`]   | `-> val`                              | Read `node.payload` as a string, look up that variable (`$payload`). |
//! | [`LoadPayload`]      | `-> val`                              | Push `node.payload` (no-scheme fallback). |
//! | [`EvalChild`]        | `-> val`                              | Evaluate child `n` of the current node (compile-cache + execute). |
//! | [`Store`]            | `val ->`                              | Pop value, push a new scope, bind name (`extend_env`). |
//! | [`StorePayloadName`] | `val ->`                              | Pop value, read `node.payload["name"]`, push scope, bind (`extend_env $payload`). |
//! | [`PopScope`]         | `->`                                  | Pop the top scope (after a `Seq` whose `fst` pushed a scope). |
//! | [`Call`]             | `callee arg... -> result`             | Apply a closure or host fn to `n` arguments (`App`). |
//! | [`ChoiceJump`]       | `either -> val`                       | Pop Either; push its value; jump to left or right target (`Choice`). |
//! | [`Jump`]             | `->`                                  | Unconditional jump. |
//! | [`NativeOp`]         | `arg... -> result`                    | Apply native op `op` to `n` args (`Arr Native`). |
//! | [`ForeignOp`]        | `arg... -> result`                    | Call host function `name` with `n` args (`Arr Foreign`). |
//! | [`FirstOp`]          | `pair -> {__pair__:[result, snd]}`    | Run inner sub-program on pair's first component; re-pair with second (`First`). |
//! | [`LoopCheck`]        | `result -> ...`                       | Inspect loop body result; route single-step vs multi-step (`Loop`). |
//! | [`Return`]           | `val ->`                              | Pop and return the top of the operand stack. |
//!
//! ## Combinator → instruction mapping
//!
//! | `SchemeV2` combinator | Compiled instructions |
//!|---|---|
//! | `Arr { op: "const", lit: Some(v), .. }` | `Push(v)` |
//! | `Arr { op: "proj_env", lit: Some(name), .. }` | `Load(name)` or `LoadPayloadVar` if name is `"$payload"` |
//! | `Arr { op: "proj_child", lit: Some(idx), .. }` | `EvalChild(idx)` |
//! | `Arr { op: "extend_env", lit: Some(name), .. }` | `Store(name)` or `StorePayloadName` if name is `"$payload"` |
//! | `Arr { op, args, source: Native, .. }` (general) | `compile(args)...`, `NativeOp(op, n)` |
//! | `Arr { op, args, source: Foreign(name), .. }` | `compile(args)...`, `ForeignOp(name, n)` |
//! | `Seq { fst, snd }` | `compile(fst)`, `compile(snd)`, [`PopScope`] if `fst` pushed a scope |
//! | `First { inner }` | `FirstOp(start, end)`, `<inner sub-program at [start, end)>` |
//! | `Choice { l, r }` | `ChoiceJump(left_target, right_target)`, `compile(l)`, `Jump(end)`, `compile(r)` |
//! | `Loop { body }` | `LoopCheck(loop_back)`, `compile(body)` (laid out as a loop body region) |
//! | `App { callee, args }` | `compile(callee)`, `compile(args)...`, `Call(n)` |
//!
//! ## Limitation: derived step ops
//!
//! The derived step ops `while_step`/`for_step`/`fix_step` (from
//! `plugin-lib-arrow-iter`) are **not** supported by the v2 bytecode VM.
//! These ops carry sub-schemes as `Arr.args` that must be evaluated with
//! specific incoming values — a pattern that requires recursive sub-program
//! execution from within a native op, which the flat bytecode model does not
//! directly support. The v2 tree-walk handler supports them via recursive
//! `eval_inner` calls. Users can construct equivalent behavior using `Loop`
//! and `Choice` directly. Supporting derived step ops in the bytecode VM is
//! future work.

use handl_language::Value;

/// A single bytecode instruction in the v2 eval VM's instruction set.
///
/// See the [module-level documentation](self) for the full ISA table,
/// combinator-to-instruction mapping, and the derived-step-op limitation.
#[derive(Debug, Clone, PartialEq)]
pub enum InstrV2 {
    /// Push a literal [`Value`] onto the operand stack.
    ///
    /// Emitted by `Arr { op: "const", lit: Some(v), .. }`.
    Push(Value),

    /// Look up a named variable in the scope stack and push its value.
    ///
    /// Emitted by `Arr { op: "proj_env", lit: Some(name), .. }` when the
    /// name is not `"$payload"`. If the variable is unbound, the name itself
    /// is pushed as a string value (matching the v2 tree-walk convention).
    Load(String),

    /// Read the current node's `payload` as a string, look up that variable
    /// name in the scope stack, and push the value.
    ///
    /// Emitted by `Arr { op: "proj_env", lit: Some("$payload"), .. }` (the
    /// actual name lives in the node's payload at runtime).
    LoadPayloadVar,

    /// Push the current node's `payload` as a value.
    ///
    /// Used by the no-scheme fallback (a node kind with no eval scheme
    /// returns its payload directly).
    LoadPayload,

    /// Evaluate the `n`-th child of the current node and push the result.
    ///
    /// Emitted by `Arr { op: "proj_child", lit: Some(idx), .. }`. At
    /// runtime, reads `node.payload["children"][n]` to obtain the child's
    /// absolute [`handl_language::AstRef`], then compiles (via the per-node
    /// cache) and executes that child node's v2 bytecode.
    EvalChild(usize),

    /// Pop a value, push a new scope, and bind the given name in that scope.
    ///
    /// Emitted by `Arr { op: "extend_env", lit: Some(name), .. }` when the
    /// name is not `"$payload"`. The matching [`PopScope`] is emitted by the
    /// enclosing `Seq` after its `snd` completes.
    Store(String),

    /// Pop a value, read `node.payload["name"]` as the bind name, push a new
    /// scope, and bind that name in the new scope.
    ///
    /// Emitted by `Arr { op: "extend_env", lit: Some("$payload"), .. }` (the
    /// actual bind name lives in the node's payload at runtime). The matching
    /// [`PopScope`] is emitted by the enclosing `Seq`.
    StorePayloadName,

    /// Pop the top scope from the scope stack.
    ///
    /// Emitted after a `Seq`'s `snd` completes, if the `fst` pushed a scope
    /// via `Store`/`StorePayloadName` (`extend_env`).
    PopScope,

    /// Pop `n` argument values, pop the callee value, apply (closure or host
    /// fn), and push the result.
    ///
    /// Emitted by `App { callee, args }`. The callee may be a closure value
    /// (`{"__closure__": true, ...}`) or a string naming a registered host
    /// function.
    Call(usize),

    /// Pop an Either value, push its carried value, and jump to `left_target`
    /// if the Either is a left, or `right_target` if it is a right.
    ///
    /// Emitted by `Choice { l, r }`. The Either shape is
    /// `{"__either__": "left"|"right", "value": v}`.
    ChoiceJump(usize, usize),

    /// Unconditionally set the instruction pointer to `target`.
    ///
    /// Emitted by `Choice` to skip the right-branch after the left-branch
    /// executes.
    Jump(usize),

    /// Pop `n` argument values, apply the native op `op`, and push the
    /// result.
    ///
    /// Emitted by `Arr { op, args, source: Native, .. }` for general native
    /// ops (not the lit-bearing special cases). The native op set matches
    /// the v2 tree-walk exactly: arithmetic (`add`/`sub`/`mul`/`div`/`mod`/
    /// `neg`), comparison (`eq`/`neq`/`lt`/`gt`/`lte`/`gte`), logic
    /// (`and`/`or`/`not`), `tag`, `id`, `closure`, and utility ops
    /// (`is_pos`/`dec`/`inc`/`half_floor`/`inc_fst`/`countdown_step`/
    /// `infinite_step`).
    NativeOp(String, usize),

    /// Pop `n` argument values, call the host function `name`, and push the
    /// result.
    ///
    /// Emitted by `Arr { op, args, source: Foreign(name), .. }`. The host
    /// function is looked up in the
    /// [`ModuleLookup`](handl_platform::ModuleLookup) registry.
    ForeignOp(String, usize),

    /// Run the `First` combinator's inner sub-program on the first component
    /// of a pair.
    ///
    /// Pop a pair `{"__pair__": [a, b]}` from the operand stack. Execute the
    /// sub-program at instruction indices `[start, end)` with `a` as the
    /// incoming value (on a fresh operand stack). The sub-program's result
    /// becomes the new first component; `b` is carried through unchanged.
    /// Push `{"__pair__": [result, b]}`. Execution continues at `end`.
    ///
    /// Using a sub-program (rather than inline `Unpair`/`Swap`/`Pair`)
    /// correctly handles inner arrows like `const` that don't consume the
    /// incoming value — the sub-program has its own fresh stack, so
    /// leftovers are discarded and only the top (the result) is returned.
    FirstOp(usize, usize),

    /// Pop two values `a` (top) and `b` (below), push the pair
    /// `{"__pair__": [b, a]}`.
    ///
    /// Used by `Loop { body }` to construct the initial pair
    /// `(incoming, Null)` from the incoming value on the stack and a pushed
    /// `Null` state seed.
    Pair,

    /// Inspect a `Loop` body result and route single-step vs multi-step
    /// execution.
    ///
    /// Pop the body result from the operand stack:
    /// - If it is an Either **left** `{"__either__": "left", "value": v}`:
    ///   push `v` and fall through (loop terminates, `v` is the output).
    /// - If it is an Either **right** `{"__either__": "right", "value": s}`:
    ///   `s` is the new state. Reconstruct the pair `(original_input, s)`,
    ///   push it, and jump to `loop_back` (the start of the body region).
    /// - If it is a pair `{"__pair__": [out, _state]}`: push `out` and fall
    ///   through (single-step mode — backward-compatible).
    ///
    /// A max-iteration counter (100_000) prevents infinite loops.
    LoopCheck(usize),

    /// Pop the top of the operand stack and return it as the program result.
    ///
    /// Every compiled program ends with `Return`.
    Return,
}
