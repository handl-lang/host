//! Bytecode instruction set for the bytecode eval engine (ADR-0044 §7).
//!
//! Each [`handl_language::Scheme`] combinator compiles to a flat sequence of
//! [`Instr`] values. The VM executes them against an operand stack and a scope
//! stack, producing values identical to the tree-walk reference interpreter.
//!
//! ## ISA overview
//!
//! | Instruction          | Stack effect                          | Description |
//! |----------------------|---------------------------------------|-------------|
//! | [`Push`]             | `→ val`                               | Push a literal value. |
//! | [`Load`]             | `→ val`                               | Look up a named variable in the scope stack. |
//! | [`LoadPayloadVar`]   | `→ val`                               | Read `node.payload` as a string, look up that variable. |
//! | [`LoadPayload`]      | `→ val`                               | Push `node.payload` (no-scheme fallback). |
//! | [`EvalChild`]        | `→ val`                               | Evaluate child `n` of the current node (compile-cache + execute). |
//! | [`Store`]            | `val →`                               | Pop value, push a new scope, bind name. |
//! | [`StorePayloadName`] | `val →`                               | Pop value, read `node.payload["name"]`, push scope, bind. |
//! | [`PopScope`]         | `→`                                   | Pop the top scope (after a `Bind` body). |
//! | [`Call`]             | `callee arg… → result`                | Apply a closure to `n` arguments. |
//! | [`JumpIfFalse`]      | `cond →`                              | Pop cond; jump to target if falsy. |
//! | [`Jump`]             | `→`                                   | Unconditional jump. |
//! | [`Prim`]             | `arg… → result`                       | Apply primitive `op` to `n` args. |
//! | [`Foreign`]          | `arg… → result`                       | Call host function `name` with `n` args. |
//! | [`Return`]           | `val →`                               | Pop and return the top of the operand stack. |

use handl_language::Value;

/// A single bytecode instruction in the eval VM's instruction set.
///
/// See the [module-level documentation](self) for the full ISA table and
/// stack-effect notation.
#[derive(Debug, Clone, PartialEq)]
pub enum Instr {
    /// Push a literal [`Value`] onto the operand stack.
    ///
    /// Emitted by [`Scheme::Lit`](handl_language::Scheme::Lit).
    Push(Value),

    /// Look up a named variable in the scope stack and push its value.
    ///
    /// Emitted by [`Scheme::Var`](handl_language::Scheme::Var) when the name
    /// is not `"$payload"`.
    Load(String),

    /// Read the current node's `payload` as a string, look up that variable
    /// name in the scope stack, and push the value.
    ///
    /// Emitted by [`Scheme::Var`](handl_language::Scheme::Var) when the name
    /// is `"$payload"` (the actual name lives in the node's payload at
    /// runtime).
    LoadPayloadVar,

    /// Push the current node's `payload` as a value.
    ///
    /// Used by the no-scheme fallback (a node kind with no eval scheme returns
    /// its payload directly).
    LoadPayload,

    /// Evaluate the `n`-th child of the current node and push the result.
    ///
    /// At runtime, reads `node.payload["children"][n]` to obtain the child's
    /// absolute [`handl_language::AstRef`], then compiles (via the per-node
    /// cache) and executes that child node's bytecode. This is the
    /// data-dispatch erasure point: the child's scheme is resolved to
    /// instructions once and cached; subsequent executions skip the
    /// `lang.eval[node.kind]` lookup (ADR-0044 §7).
    EvalChild(usize),

    /// Pop a value, push a new scope, and bind the given name in that scope.
    ///
    /// Emitted by [`Scheme::Bind`](handl_language::Scheme::Bind) when the name
    /// is not `"$payload"`. Always paired with a later [`PopScope`].
    Store(String),

    /// Pop a value, read `node.payload["name"]` as the bind name, push a new
    /// scope, and bind that name in the new scope.
    ///
    /// Emitted by [`Scheme::Bind`](handl_language::Scheme::Bind) when the name
    /// is `"$payload"` (the actual bind name lives in the node's payload at
    /// runtime). Always paired with a later [`PopScope`].
    StorePayloadName,

    /// Pop the top scope from the scope stack.
    ///
    /// Emitted after a [`Store`] / [`StorePayloadName`] body completes,
    /// matching the tree-walk's `pop_scope` after a `Bind`.
    PopScope,

    /// Pop `n` argument values, pop the callee value, apply the closure, and
    /// push the result.
    ///
    /// Emitted by [`Scheme::App`](handl_language::Scheme::App). The callee
    /// must be a closure value (`{"__closure__": true, …}`).
    Call(usize),

    /// Pop a condition value; if it is falsy, set the instruction pointer to
    /// `target`.
    ///
    /// Emitted by [`Scheme::Branch`](handl_language::Scheme::Branch) for the
    /// condition. Falsy = `false`, `null`, `0`, or `""`.
    JumpIfFalse(usize),

    /// Unconditionally set the instruction pointer to `target`.
    ///
    /// Emitted by [`Scheme::Branch`](handl_language::Scheme::Branch) to skip
    /// the else-branch after the then-branch executes.
    Jump(usize),

    /// Pop `n` argument values, apply the primitive operation `op`, and push
    /// the result.
    ///
    /// Emitted by [`Scheme::Prim`](handl_language::Scheme::Prim). The
    /// primitive set matches the tree-walk exactly: `add`, `sub`, `mul`,
    /// `div`, `eq`, `lt`, `gt`, `not`, `and`, `or`, `closure`.
    Prim(String, usize),

    /// Pop `n` argument values, call the host function `name`, and push the
    /// result.
    ///
    /// Emitted by [`Scheme::Foreign`](handl_language::Scheme::Foreign). The
    /// host function is looked up in the [`HostFunctions`](handl_plugin_engine_eval::HostFunctions)
    /// registry.
    Foreign(String, usize),

    /// Push a new empty scope for the `vau` parameter bindings (the vau
    /// escape, ADR-0041 §6). Emitted once before the [`VauBind`]s of a
    /// [`Scheme::Vau`](handl_language::Scheme::Vau). Always paired with a
    /// later [`PopScope`] after the vau body completes.
    VauEnter,

    /// Bind the `n`-th child of the current node — **unevaluated** — to the
    /// given name in the current scope (the `vau` escape, ADR-0041 §6).
    ///
    /// Reads `node.payload["children"][child_n]` to get the child's
    /// absolute [`handl_language::AstRef`], extracts the child's subtree
    /// into a standalone [`handl_language::Ast`], wraps it as a tagged
    /// `__ast__` value, and binds `name` to that value in the current
    /// (innermost) scope. The child is **not** evaluated. Preceded by a
    /// [`VauEnter`] (which pushed the scope) and followed by a [`PopScope`]
    /// after the vau body.
    ///
    /// Emitted by [`Scheme::Vau`](handl_language::Scheme::Vau) once per
    /// parameter.
    VauBind(String, usize),

    /// Pop the top of the operand stack and return it as the program result.
    ///
    /// Every compiled program ends with `Return`.
    Return,
}
