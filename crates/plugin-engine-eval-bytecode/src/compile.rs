//! `Scheme` -> v2 bytecode compilation (ADR-0052 Phase B).
//!
//! The [`compile`] function translates a
//! [`Scheme`](handl_language::Scheme) combinator tree into a
//! flat [`Vec<Instr>`](crate::Instr) program. Each combinator maps to a
//! fixed instruction sequence -- see the
//! [`instr`](crate::instr) module for the full mapping table.
//!
//! Every compiled program ends with [`Instr::Return`]. Sub-schemes are
//! inlined (their instructions are appended without a trailing `Return`) so
//! that exactly one value remains on the operand stack when the program
//! completes.
//!
//! ## Scope discipline
//!
//! Only `extend_env` pushes a scope. [`compile_into`] returns `true` when
//! the compiled code pushes a scope that the caller is responsible for
//! popping. `Seq` pops the scope its `fst` reported after `snd` completes
//! (emitting [`Instr::PopScope`]), and propagates whatever `snd` reported.
//! `First`/`Choice`/`Loop`/`App` pop any scope their sub-evaluation pushed
//! immediately after it completes. A bare `extend_env` at the top level is
//! balanced by [`compile`] appending a [`Instr::PopScope`] before
//! `Return`. This mirrors the tree-walk's `did_push` discipline exactly.

use handl_language::{ArrSource, Scheme};
use handl_language::{AstNode, LangError, Value};

use crate::instr::Instr;

/// Compile a [`Scheme`] into a v2 bytecode program.
///
/// The `node` parameter is reserved for future compile-time optimizations.
/// The current ISA handles all `$payload` conventions at runtime via
/// dedicated instruction variants, so the node is not inspected during
/// compilation.
///
/// # Errors
///
/// Returns [`LangError::Engine`] if a sub-scheme fails to compile. In
/// practice, every well-formed [`Scheme`] variant compiles successfully.
pub fn compile(scheme: &Scheme, _node: &AstNode) -> Result<Vec<Instr>, LangError> {
    let mut out = Vec::new();
    let pushed = compile_into(scheme, &mut out)?;
    // A bare extend_env (no enclosing Seq to pop it) is balanced here.
    if pushed {
        out.push(Instr::PopScope);
    }
    out.push(Instr::Return);
    Ok(out)
}

/// Recursively append instructions for `scheme` into `out`.
///
/// After execution of the appended instructions, exactly one value remains on
/// the operand stack. No trailing [`Instr::Return`] is emitted -- the
/// caller ([`compile`]) adds it once at the top level.
///
/// Returns `Ok(true)` if the compiled code pushes a scope that the caller is
/// responsible for popping (i.e. `extend_env` was emitted). This mirrors the
/// tree-walk's `did_push` discipline.
fn compile_into(scheme: &Scheme, out: &mut Vec<Instr>) -> Result<bool, LangError> {
    match scheme {
        Scheme::Arr {
            op,
            args,
            source,
            lit,
        } => compile_arr(op, args, source, lit.as_ref(), out),

        Scheme::Seq { fst, snd } => {
            let fst_pushed = compile_into(fst, out)?;
            let snd_pushed = compile_into(snd, out)?;
            // Pop the scope `fst` reported (e.g. extend_env as fst), matching
            // the tree-walk's Seq scope discipline.
            if fst_pushed {
                out.push(Instr::PopScope);
            }
            // Propagate snd_pushed: an extend_env in snd position (the Bind
            // lowering shape) is popped by the *enclosing* Seq.
            Ok(snd_pushed)
        }

        Scheme::First { inner } => {
            // FirstOp(start, end) — the inner sub-program runs on a fresh
            // stack with the pair's first component as incoming. This
            // correctly handles inner arrows like `const` that don't consume
            // the incoming (the sub-program's fresh stack discards
            // leftovers; only the top is returned as the result).
            let firstop_pos = out.len();
            out.push(Instr::FirstOp(0, 0)); // placeholder
            let inner_start = out.len();
            let inner_pushed = compile_into(inner, out)?;
            if inner_pushed {
                out.push(Instr::PopScope);
            }
            let inner_end = out.len();
            out[firstop_pos] = Instr::FirstOp(inner_start, inner_end);
            Ok(false)
        }

        Scheme::Choice { l, r } => {
            // ChoiceJump(left_target, right_target) -- placeholder.
            let cj_pos = out.len();
            out.push(Instr::ChoiceJump(0, 0));
            // Left branch.
            let left_target = out.len();
            let l_pushed = compile_into(l, out)?;
            if l_pushed {
                out.push(Instr::PopScope);
            }
            let left_jump_pos = out.len();
            out.push(Instr::Jump(0)); // placeholder -- skip right branch
            // Right branch.
            let right_target = out.len();
            let r_pushed = compile_into(r, out)?;
            if r_pushed {
                out.push(Instr::PopScope);
            }
            let end = out.len();
            // Patch ChoiceJump and the left-branch Jump.
            out[cj_pos] = Instr::ChoiceJump(left_target, right_target);
            out[left_jump_pos] = Instr::Jump(end);
            Ok(false)
        }

        Scheme::Loop { body } => {
            // Loop layout:
            //   Push(Null)          -- initial state
            //   Pair                -- pair(incoming, Null) on stack
            //   <loop_back>: <body code>
            //   LoopCheck(<loop_back>)
            //
            // The incoming value is on the stack from the enclosing context.
            // We pair it with Null (the initial state seed), then the body
            // consumes the pair and leaves its result. LoopCheck inspects
            // the result and either terminates (left/pair) or loops back
            // (right) with the new state.
            out.push(Instr::Push(Value::Null));
            out.push(Instr::Pair);
            let loop_back = out.len();
            let body_pushed = compile_into(body, out)?;
            // The tree-walk pops any scope the body pushed after each
            // iteration. In the VM, we emit PopScope after the body code
            // but before LoopCheck, so it runs each iteration.
            if body_pushed {
                out.push(Instr::PopScope);
            }
            out.push(Instr::LoopCheck(loop_back));
            Ok(false)
        }

        Scheme::App { callee, args } => {
            let callee_pushed = compile_into(callee, out)?;
            if callee_pushed {
                out.push(Instr::PopScope);
            }
            for arg in args {
                let arg_pushed = compile_into(arg, out)?;
                if arg_pushed {
                    out.push(Instr::PopScope);
                }
            }
            out.push(Instr::Call(args.len()));
            Ok(false)
        }
    }
}

/// Compile an `Arr` combinator. Returns `Ok(true)` if this Arr pushes a scope
/// (`extend_env`), so the caller can emit `PopScope`.
fn compile_arr(
    op: &str,
    args: &[Scheme],
    source: &ArrSource,
    lit: Option<&Value>,
    out: &mut Vec<Instr>,
) -> Result<bool, LangError> {
    // Special-case the lit-bearing ops.
    match op {
        "const" => {
            out.push(Instr::Push(lit.cloned().unwrap_or(Value::Null)));
            return Ok(false);
        }
        "proj_env" => {
            let name = lit
                .and_then(|v| v.as_str())
                .ok_or_else(|| LangError::Engine("proj_env: missing name literal".to_string()))?;
            if name == "$payload" {
                out.push(Instr::LoadPayloadVar);
            } else {
                out.push(Instr::Load(name.to_string()));
            }
            return Ok(false);
        }
        "proj_child" => {
            let idx = lit
                .and_then(|v| v.as_u64())
                .ok_or_else(|| LangError::Engine("proj_child: missing index literal".to_string()))?;
            out.push(Instr::EvalChild(idx as usize));
            return Ok(false);
        }
        "extend_env" => {
            let name = lit
                .and_then(|v| v.as_str())
                .ok_or_else(|| LangError::Engine("extend_env: missing name literal".to_string()))?;
            if name == "$payload" {
                out.push(Instr::StorePayloadName);
            } else {
                out.push(Instr::Store(name.to_string()));
            }
            return Ok(true); // pushed a scope -- caller pops
        }
        _ => {}
    }

    // General native or foreign op: compile args, then emit NativeOp/ForeignOp.
    for arg in args {
        let arg_pushed = compile_into(arg, out)?;
        if arg_pushed {
            out.push(Instr::PopScope);
        }
    }
    match source {
        ArrSource::Native => {
            out.push(Instr::NativeOp(op.to_string(), args.len()));
        }
        ArrSource::Foreign(name) => {
            out.push(Instr::ForeignOp(name.clone(), args.len()));
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::Scheme;
    use handl_language::AstNode;
    use serde_json::json;

    /// A dummy node used by compilation tests (the node is not inspected).
    fn dummy_node() -> AstNode {
        AstNode::new("test")
    }

    // --- Arr compilation tests ---

    #[test]
    fn compile_const() {
        let s = Scheme::arr_native_lit("const", vec![], json!(42));
        let instrs = compile(&s, &dummy_node()).unwrap();
        assert_eq!(instrs, vec![Instr::Push(json!(42)), Instr::Return]);
    }

    #[test]
    fn compile_const_no_lit() {
        let s = Scheme::arr_native("const", vec![]);
        let instrs = compile(&s, &dummy_node()).unwrap();
        assert_eq!(instrs, vec![Instr::Push(Value::Null), Instr::Return]);
    }

    #[test]
    fn compile_proj_env_named() {
        let s = Scheme::arr_native_lit("proj_env", vec![], "x");
        let instrs = compile(&s, &dummy_node()).unwrap();
        assert_eq!(instrs, vec![Instr::Load("x".into()), Instr::Return]);
    }

    #[test]
    fn compile_proj_env_payload() {
        let s = Scheme::arr_native_lit("proj_env", vec![], "$payload");
        let instrs = compile(&s, &dummy_node()).unwrap();
        assert_eq!(instrs, vec![Instr::LoadPayloadVar, Instr::Return]);
    }

    #[test]
    fn compile_proj_child() {
        let s = Scheme::arr_native_lit("proj_child", vec![], json!(2));
        let instrs = compile(&s, &dummy_node()).unwrap();
        assert_eq!(instrs, vec![Instr::EvalChild(2), Instr::Return]);
    }

    #[test]
    fn compile_extend_env_named() {
        let s = Scheme::arr_native_lit("extend_env", vec![], "x");
        let instrs = compile(&s, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![Instr::Store("x".into()), Instr::PopScope, Instr::Return]
        );
    }

    #[test]
    fn compile_extend_env_payload() {
        let s = Scheme::arr_native_lit("extend_env", vec![], "$payload");
        let instrs = compile(&s, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![Instr::StorePayloadName, Instr::PopScope, Instr::Return]
        );
    }

    #[test]
    fn compile_native_op() {
        let s = Scheme::arr_native(
            "add",
            vec![
                Scheme::arr_native_lit("proj_child", vec![], json!(0)),
                Scheme::arr_native_lit("proj_child", vec![], json!(1)),
            ],
        );
        let instrs = compile(&s, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                Instr::EvalChild(0),
                Instr::EvalChild(1),
                Instr::NativeOp("add".into(), 2),
                Instr::Return,
            ]
        );
    }

    #[test]
    fn compile_foreign_op() {
        let s = Scheme::arr_foreign(
            "double",
            vec![Scheme::arr_native_lit("proj_child", vec![], json!(0))],
        );
        let instrs = compile(&s, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                Instr::EvalChild(0),
                Instr::ForeignOp("double".into(), 1),
                Instr::Return,
            ]
        );
    }

    // --- Seq compilation tests ---

    #[test]
    fn compile_seq_no_scope() {
        let s = Scheme::seq(
            Scheme::arr_native_lit("const", vec![], json!(5)),
            Scheme::arr_native("id", vec![]),
        );
        let instrs = compile(&s, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                Instr::Push(json!(5)),
                Instr::NativeOp("id".into(), 0),
                Instr::Return,
            ]
        );
    }

    #[test]
    fn compile_seq_with_extend_env_pops_scope() {
        // Seq(Seq(const 5, extend_env "x"), proj_env "x") -- the inner Seq's
        // fst (const) does not push a scope. The inner Seq's snd (extend_env)
        // pushes a scope, which propagates to the outer Seq. The outer Seq
        // pops after its snd (proj_env) completes.
        let s = Scheme::seq(
            Scheme::seq(
                Scheme::arr_native_lit("const", vec![], json!(5)),
                Scheme::arr_native_lit("extend_env", vec![], "x"),
            ),
            Scheme::arr_native_lit("proj_env", vec![], "x"),
        );
        let instrs = compile(&s, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                Instr::Push(json!(5)),     // 0: const 5
                Instr::Store("x".into()),  // 1: extend_env "x"
                Instr::Load("x".into()),   // 2: proj_env "x"
                Instr::PopScope,           // 3: pop scope (outer Seq)
                Instr::Return,             // 4
            ]
        );
    }

    // --- First compilation tests ---

    #[test]
    fn compile_first() {
        let s = Scheme::first(Scheme::arr_native("id", vec![]));
        let instrs = compile(&s, &dummy_node()).unwrap();
        // FirstOp(1, 2) — inner sub-program at index 1..2 is the `id` op.
        assert_eq!(
            instrs,
            vec![
                Instr::FirstOp(1, 2),
                Instr::NativeOp("id".into(), 0),
                Instr::Return,
            ]
        );
    }

    // --- Choice compilation tests ---

    #[test]
    fn compile_choice() {
        let s = Scheme::choice(
            Scheme::arr_native_lit("const", vec![], json!(1)),
            Scheme::arr_native_lit("const", vec![], json!(2)),
        );
        let instrs = compile(&s, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                Instr::ChoiceJump(1, 3), // 0: route left/right
                Instr::Push(json!(1)),    // 1: left branch
                Instr::Jump(4),           // 2: skip right
                Instr::Push(json!(2)),    // 3: right branch
                Instr::Return,            // 4: end
            ]
        );
    }

    // --- Loop compilation tests ---

    #[test]
    fn compile_loop() {
        let s = Scheme::loop_feedback(Scheme::arr_native("inc_fst", vec![]));
        let instrs = compile(&s, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                Instr::Push(Value::Null),            // 0: initial state
                Instr::Pair,                          // 1: pair(incoming, Null)
                Instr::NativeOp("inc_fst".into(), 0), // 2: body (loop_back = 2)
                Instr::LoopCheck(2),                  // 3: check + loop back
                Instr::Return,                        // 4
            ]
        );
    }

    // --- App compilation tests ---

    #[test]
    fn compile_app() {
        let s = Scheme::app(
            Scheme::arr_native_lit("proj_env", vec![], "f"),
            vec![Scheme::arr_native_lit("const", vec![], json!(41))],
        );
        let instrs = compile(&s, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                Instr::Load("f".into()),
                Instr::Push(json!(41)),
                Instr::Call(1),
                Instr::Return,
            ]
        );
    }

    #[test]
    fn compile_app_no_args() {
        let s = Scheme::app(
            Scheme::arr_native_lit("proj_env", vec![], "f"),
            vec![],
        );
        let instrs = compile(&s, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![Instr::Load("f".into()), Instr::Call(0), Instr::Return]
        );
    }

    // --- Branch lowering (Seq + tag + Choice) ---

    #[test]
    fn compile_branch_lowering() {
        // Seq(Seq(const true, tag), Choice(const "yes", const "no"))
        let s = Scheme::seq(
            Scheme::seq(
                Scheme::arr_native_lit("const", vec![], json!(true)),
                Scheme::arr_native("tag", vec![]),
            ),
            Scheme::choice(
                Scheme::arr_native_lit("const", vec![], "yes"),
                Scheme::arr_native_lit("const", vec![], "no"),
            ),
        );
        let instrs = compile(&s, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                Instr::Push(json!(true)),         // 0: const true
                Instr::NativeOp("tag".into(), 0),  // 1: tag
                Instr::ChoiceJump(3, 5),           // 2: route left/right
                Instr::Push(json!("yes")),         // 3: left (then)
                Instr::Jump(6),                    // 4: skip right
                Instr::Push(json!("no")),          // 5: right (else)
                Instr::Return,                     // 6: end
            ]
        );
    }
}
