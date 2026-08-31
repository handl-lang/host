//! `SchemeV2` -> v2 bytecode compilation (ADR-0052 Phase B).
//!
//! The [`compile_v2`] function translates a
//! [`SchemeV2`](handl_language::scheme_v2::SchemeV2) combinator tree into a
//! flat [`Vec<InstrV2>`](crate::InstrV2) program. Each combinator maps to a
//! fixed instruction sequence -- see the
//! [`instr_v2`](crate::instr_v2) module for the full mapping table.
//!
//! Every compiled program ends with [`InstrV2::Return`]. Sub-schemes are
//! inlined (their instructions are appended without a trailing `Return`) so
//! that exactly one value remains on the operand stack when the program
//! completes.
//!
//! ## Scope discipline
//!
//! Only `extend_env` pushes a scope. [`compile_into_v2`] returns `true` when
//! the compiled code pushes a scope that the caller is responsible for
//! popping. `Seq` pops the scope its `fst` reported after `snd` completes
//! (emitting [`InstrV2::PopScope`]), and propagates whatever `snd` reported.
//! `First`/`Choice`/`Loop`/`App` pop any scope their sub-evaluation pushed
//! immediately after it completes. A bare `extend_env` at the top level is
//! balanced by [`compile_v2`] appending a [`InstrV2::PopScope`] before
//! `Return`. This mirrors the tree-walk's `did_push` discipline exactly.

use handl_language::scheme_v2::{ArrSource, SchemeV2};
use handl_language::{AstNode, LangError, Value};

use crate::instr_v2::InstrV2;

/// Compile a [`SchemeV2`] into a v2 bytecode program.
///
/// The `node` parameter is reserved for future compile-time optimizations.
/// The current ISA handles all `$payload` conventions at runtime via
/// dedicated instruction variants, so the node is not inspected during
/// compilation.
///
/// # Errors
///
/// Returns [`LangError::Engine`] if a sub-scheme fails to compile. In
/// practice, every well-formed [`SchemeV2`] variant compiles successfully.
pub fn compile_v2(scheme: &SchemeV2, _node: &AstNode) -> Result<Vec<InstrV2>, LangError> {
    let mut out = Vec::new();
    let pushed = compile_into_v2(scheme, &mut out)?;
    // A bare extend_env (no enclosing Seq to pop it) is balanced here.
    if pushed {
        out.push(InstrV2::PopScope);
    }
    out.push(InstrV2::Return);
    Ok(out)
}

/// Recursively append instructions for `scheme` into `out`.
///
/// After execution of the appended instructions, exactly one value remains on
/// the operand stack. No trailing [`InstrV2::Return`] is emitted -- the
/// caller ([`compile_v2`]) adds it once at the top level.
///
/// Returns `Ok(true)` if the compiled code pushes a scope that the caller is
/// responsible for popping (i.e. `extend_env` was emitted). This mirrors the
/// tree-walk's `did_push` discipline.
fn compile_into_v2(scheme: &SchemeV2, out: &mut Vec<InstrV2>) -> Result<bool, LangError> {
    match scheme {
        SchemeV2::Arr {
            op,
            args,
            source,
            lit,
        } => compile_arr_v2(op, args, source, lit.as_ref(), out),

        SchemeV2::Seq { fst, snd } => {
            let fst_pushed = compile_into_v2(fst, out)?;
            let snd_pushed = compile_into_v2(snd, out)?;
            // Pop the scope `fst` reported (e.g. extend_env as fst), matching
            // the tree-walk's Seq scope discipline.
            if fst_pushed {
                out.push(InstrV2::PopScope);
            }
            // Propagate snd_pushed: an extend_env in snd position (the Bind
            // lowering shape) is popped by the *enclosing* Seq.
            Ok(snd_pushed)
        }

        SchemeV2::First { inner } => {
            // FirstOp(start, end) — the inner sub-program runs on a fresh
            // stack with the pair's first component as incoming. This
            // correctly handles inner arrows like `const` that don't consume
            // the incoming (the sub-program's fresh stack discards
            // leftovers; only the top is returned as the result).
            let firstop_pos = out.len();
            out.push(InstrV2::FirstOp(0, 0)); // placeholder
            let inner_start = out.len();
            let inner_pushed = compile_into_v2(inner, out)?;
            if inner_pushed {
                out.push(InstrV2::PopScope);
            }
            let inner_end = out.len();
            out[firstop_pos] = InstrV2::FirstOp(inner_start, inner_end);
            Ok(false)
        }

        SchemeV2::Choice { l, r } => {
            // ChoiceJump(left_target, right_target) -- placeholder.
            let cj_pos = out.len();
            out.push(InstrV2::ChoiceJump(0, 0));
            // Left branch.
            let left_target = out.len();
            let l_pushed = compile_into_v2(l, out)?;
            if l_pushed {
                out.push(InstrV2::PopScope);
            }
            let left_jump_pos = out.len();
            out.push(InstrV2::Jump(0)); // placeholder -- skip right branch
            // Right branch.
            let right_target = out.len();
            let r_pushed = compile_into_v2(r, out)?;
            if r_pushed {
                out.push(InstrV2::PopScope);
            }
            let end = out.len();
            // Patch ChoiceJump and the left-branch Jump.
            out[cj_pos] = InstrV2::ChoiceJump(left_target, right_target);
            out[left_jump_pos] = InstrV2::Jump(end);
            Ok(false)
        }

        SchemeV2::Loop { body } => {
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
            out.push(InstrV2::Push(Value::Null));
            out.push(InstrV2::Pair);
            let loop_back = out.len();
            let body_pushed = compile_into_v2(body, out)?;
            // The tree-walk pops any scope the body pushed after each
            // iteration. In the VM, we emit PopScope after the body code
            // but before LoopCheck, so it runs each iteration.
            if body_pushed {
                out.push(InstrV2::PopScope);
            }
            out.push(InstrV2::LoopCheck(loop_back));
            Ok(false)
        }

        SchemeV2::App { callee, args } => {
            let callee_pushed = compile_into_v2(callee, out)?;
            if callee_pushed {
                out.push(InstrV2::PopScope);
            }
            for arg in args {
                let arg_pushed = compile_into_v2(arg, out)?;
                if arg_pushed {
                    out.push(InstrV2::PopScope);
                }
            }
            out.push(InstrV2::Call(args.len()));
            Ok(false)
        }
    }
}

/// Compile an `Arr` combinator. Returns `Ok(true)` if this Arr pushes a scope
/// (`extend_env`), so the caller can emit `PopScope`.
fn compile_arr_v2(
    op: &str,
    args: &[SchemeV2],
    source: &ArrSource,
    lit: Option<&Value>,
    out: &mut Vec<InstrV2>,
) -> Result<bool, LangError> {
    // Special-case the lit-bearing ops.
    match op {
        "const" => {
            out.push(InstrV2::Push(lit.cloned().unwrap_or(Value::Null)));
            return Ok(false);
        }
        "proj_env" => {
            let name = lit
                .and_then(|v| v.as_str())
                .ok_or_else(|| LangError::Engine("proj_env: missing name literal".to_string()))?;
            if name == "$payload" {
                out.push(InstrV2::LoadPayloadVar);
            } else {
                out.push(InstrV2::Load(name.to_string()));
            }
            return Ok(false);
        }
        "proj_child" => {
            let idx = lit
                .and_then(|v| v.as_u64())
                .ok_or_else(|| LangError::Engine("proj_child: missing index literal".to_string()))?;
            out.push(InstrV2::EvalChild(idx as usize));
            return Ok(false);
        }
        "extend_env" => {
            let name = lit
                .and_then(|v| v.as_str())
                .ok_or_else(|| LangError::Engine("extend_env: missing name literal".to_string()))?;
            if name == "$payload" {
                out.push(InstrV2::StorePayloadName);
            } else {
                out.push(InstrV2::Store(name.to_string()));
            }
            return Ok(true); // pushed a scope -- caller pops
        }
        _ => {}
    }

    // General native or foreign op: compile args, then emit NativeOp/ForeignOp.
    for arg in args {
        let arg_pushed = compile_into_v2(arg, out)?;
        if arg_pushed {
            out.push(InstrV2::PopScope);
        }
    }
    match source {
        ArrSource::Native => {
            out.push(InstrV2::NativeOp(op.to_string(), args.len()));
        }
        ArrSource::Foreign(name) => {
            out.push(InstrV2::ForeignOp(name.clone(), args.len()));
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::scheme_v2::SchemeV2;
    use handl_language::AstNode;
    use serde_json::json;

    /// A dummy node used by compilation tests (the node is not inspected).
    fn dummy_node() -> AstNode {
        AstNode::new("test")
    }

    // --- Arr compilation tests ---

    #[test]
    fn compile_const() {
        let s = SchemeV2::arr_native_lit("const", vec![], json!(42));
        let instrs = compile_v2(&s, &dummy_node()).unwrap();
        assert_eq!(instrs, vec![InstrV2::Push(json!(42)), InstrV2::Return]);
    }

    #[test]
    fn compile_const_no_lit() {
        let s = SchemeV2::arr_native("const", vec![]);
        let instrs = compile_v2(&s, &dummy_node()).unwrap();
        assert_eq!(instrs, vec![InstrV2::Push(Value::Null), InstrV2::Return]);
    }

    #[test]
    fn compile_proj_env_named() {
        let s = SchemeV2::arr_native_lit("proj_env", vec![], "x");
        let instrs = compile_v2(&s, &dummy_node()).unwrap();
        assert_eq!(instrs, vec![InstrV2::Load("x".into()), InstrV2::Return]);
    }

    #[test]
    fn compile_proj_env_payload() {
        let s = SchemeV2::arr_native_lit("proj_env", vec![], "$payload");
        let instrs = compile_v2(&s, &dummy_node()).unwrap();
        assert_eq!(instrs, vec![InstrV2::LoadPayloadVar, InstrV2::Return]);
    }

    #[test]
    fn compile_proj_child() {
        let s = SchemeV2::arr_native_lit("proj_child", vec![], json!(2));
        let instrs = compile_v2(&s, &dummy_node()).unwrap();
        assert_eq!(instrs, vec![InstrV2::EvalChild(2), InstrV2::Return]);
    }

    #[test]
    fn compile_extend_env_named() {
        let s = SchemeV2::arr_native_lit("extend_env", vec![], "x");
        let instrs = compile_v2(&s, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![InstrV2::Store("x".into()), InstrV2::PopScope, InstrV2::Return]
        );
    }

    #[test]
    fn compile_extend_env_payload() {
        let s = SchemeV2::arr_native_lit("extend_env", vec![], "$payload");
        let instrs = compile_v2(&s, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![InstrV2::StorePayloadName, InstrV2::PopScope, InstrV2::Return]
        );
    }

    #[test]
    fn compile_native_op() {
        let s = SchemeV2::arr_native(
            "add",
            vec![
                SchemeV2::arr_native_lit("proj_child", vec![], json!(0)),
                SchemeV2::arr_native_lit("proj_child", vec![], json!(1)),
            ],
        );
        let instrs = compile_v2(&s, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                InstrV2::EvalChild(0),
                InstrV2::EvalChild(1),
                InstrV2::NativeOp("add".into(), 2),
                InstrV2::Return,
            ]
        );
    }

    #[test]
    fn compile_foreign_op() {
        let s = SchemeV2::arr_foreign(
            "double",
            vec![SchemeV2::arr_native_lit("proj_child", vec![], json!(0))],
        );
        let instrs = compile_v2(&s, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                InstrV2::EvalChild(0),
                InstrV2::ForeignOp("double".into(), 1),
                InstrV2::Return,
            ]
        );
    }

    // --- Seq compilation tests ---

    #[test]
    fn compile_seq_no_scope() {
        let s = SchemeV2::seq(
            SchemeV2::arr_native_lit("const", vec![], json!(5)),
            SchemeV2::arr_native("id", vec![]),
        );
        let instrs = compile_v2(&s, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                InstrV2::Push(json!(5)),
                InstrV2::NativeOp("id".into(), 0),
                InstrV2::Return,
            ]
        );
    }

    #[test]
    fn compile_seq_with_extend_env_pops_scope() {
        // Seq(Seq(const 5, extend_env "x"), proj_env "x") -- the inner Seq's
        // fst (const) does not push a scope. The inner Seq's snd (extend_env)
        // pushes a scope, which propagates to the outer Seq. The outer Seq
        // pops after its snd (proj_env) completes.
        let s = SchemeV2::seq(
            SchemeV2::seq(
                SchemeV2::arr_native_lit("const", vec![], json!(5)),
                SchemeV2::arr_native_lit("extend_env", vec![], "x"),
            ),
            SchemeV2::arr_native_lit("proj_env", vec![], "x"),
        );
        let instrs = compile_v2(&s, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                InstrV2::Push(json!(5)),     // 0: const 5
                InstrV2::Store("x".into()),  // 1: extend_env "x"
                InstrV2::Load("x".into()),   // 2: proj_env "x"
                InstrV2::PopScope,           // 3: pop scope (outer Seq)
                InstrV2::Return,             // 4
            ]
        );
    }

    // --- First compilation tests ---

    #[test]
    fn compile_first() {
        let s = SchemeV2::first(SchemeV2::arr_native("id", vec![]));
        let instrs = compile_v2(&s, &dummy_node()).unwrap();
        // FirstOp(1, 2) — inner sub-program at index 1..2 is the `id` op.
        assert_eq!(
            instrs,
            vec![
                InstrV2::FirstOp(1, 2),
                InstrV2::NativeOp("id".into(), 0),
                InstrV2::Return,
            ]
        );
    }

    // --- Choice compilation tests ---

    #[test]
    fn compile_choice() {
        let s = SchemeV2::choice(
            SchemeV2::arr_native_lit("const", vec![], json!(1)),
            SchemeV2::arr_native_lit("const", vec![], json!(2)),
        );
        let instrs = compile_v2(&s, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                InstrV2::ChoiceJump(1, 3), // 0: route left/right
                InstrV2::Push(json!(1)),    // 1: left branch
                InstrV2::Jump(4),           // 2: skip right
                InstrV2::Push(json!(2)),    // 3: right branch
                InstrV2::Return,            // 4: end
            ]
        );
    }

    // --- Loop compilation tests ---

    #[test]
    fn compile_loop() {
        let s = SchemeV2::loop_feedback(SchemeV2::arr_native("inc_fst", vec![]));
        let instrs = compile_v2(&s, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                InstrV2::Push(Value::Null),            // 0: initial state
                InstrV2::Pair,                          // 1: pair(incoming, Null)
                InstrV2::NativeOp("inc_fst".into(), 0), // 2: body (loop_back = 2)
                InstrV2::LoopCheck(2),                  // 3: check + loop back
                InstrV2::Return,                        // 4
            ]
        );
    }

    // --- App compilation tests ---

    #[test]
    fn compile_app() {
        let s = SchemeV2::app(
            SchemeV2::arr_native_lit("proj_env", vec![], "f"),
            vec![SchemeV2::arr_native_lit("const", vec![], json!(41))],
        );
        let instrs = compile_v2(&s, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                InstrV2::Load("f".into()),
                InstrV2::Push(json!(41)),
                InstrV2::Call(1),
                InstrV2::Return,
            ]
        );
    }

    #[test]
    fn compile_app_no_args() {
        let s = SchemeV2::app(
            SchemeV2::arr_native_lit("proj_env", vec![], "f"),
            vec![],
        );
        let instrs = compile_v2(&s, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![InstrV2::Load("f".into()), InstrV2::Call(0), InstrV2::Return]
        );
    }

    // --- Branch lowering (Seq + tag + Choice) ---

    #[test]
    fn compile_branch_lowering() {
        // Seq(Seq(const true, tag), Choice(const "yes", const "no"))
        let s = SchemeV2::seq(
            SchemeV2::seq(
                SchemeV2::arr_native_lit("const", vec![], json!(true)),
                SchemeV2::arr_native("tag", vec![]),
            ),
            SchemeV2::choice(
                SchemeV2::arr_native_lit("const", vec![], "yes"),
                SchemeV2::arr_native_lit("const", vec![], "no"),
            ),
        );
        let instrs = compile_v2(&s, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                InstrV2::Push(json!(true)),         // 0: const true
                InstrV2::NativeOp("tag".into(), 0),  // 1: tag
                InstrV2::ChoiceJump(3, 5),           // 2: route left/right
                InstrV2::Push(json!("yes")),         // 3: left (then)
                InstrV2::Jump(6),                    // 4: skip right
                InstrV2::Push(json!("no")),          // 5: right (else)
                InstrV2::Return,                     // 6: end
            ]
        );
    }
}
