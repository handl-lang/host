//! `Scheme` → bytecode compilation (ADR-0044 §7).
//!
//! The [`compile`] function translates a [`Scheme`] combinator tree into a flat
//! [`Vec<Instr>`](crate::Instr) program. Each combinator maps to a fixed
//! instruction sequence:
//!
//! - [`Scheme::Lit(v)`] → `[Push(v)]`
//! - [`Scheme::Var("x")`] → `[Load("x")]`
//! - [`Scheme::Var("$payload")]`] → `[LoadPayloadVar]`
//! - [`Scheme::Child(n)`] → `[EvalChild(n)]`
//! - [`Scheme::Bind`] → `compile(value)`, `Store(name)`, `compile(body)`,
//!   `PopScope`
//! - [`Scheme::App`] → `compile(callee)`, `compile(args)…`, `Call(n)`
//! - [`Scheme::Branch`] → `compile(cond)`, `JumpIfFalse(else)`,
//!   `compile(then)`, `Jump(end)`, `compile(else)`
//! - [`Scheme::Prim`] → `compile(args)…`, `Prim(op, n)`
//! - [`Scheme::Foreign`] → `compile(args)…`, `Foreign(name, n)`
//! - [`Scheme::Vau`] → `VauEnter`, `VauBind(p_i, i)…`, `compile(body)`,
//!   `PopScope`
//!
//! Every compiled program ends with [`Instr::Return`]. Sub-schemes are
//! inlined (their instructions are appended without a trailing `Return`) so
//! that exactly one value remains on the operand stack when the program
//! completes.

use handl_language::{AstNode, LangError, Scheme};

use crate::instr::Instr;

/// Compile a [`Scheme`] into a bytecode program.
///
/// The `node` parameter is reserved for future compile-time optimizations
/// (e.g. constant folding based on payload). The current ISA handles all
/// `$payload` conventions at runtime via dedicated instruction variants, so
/// the node is not inspected during compilation.
///
/// # Errors
///
/// Returns [`LangError::Engine`] if a sub-scheme fails to compile. In
/// practice, every well-typed [`Scheme`] variant compiles successfully; the
/// `Result` is kept for forward compatibility with future scheme versions
/// that may introduce unsupported combinators.
pub fn compile(scheme: &Scheme, _node: &AstNode) -> Result<Vec<Instr>, LangError> {
    let mut out = Vec::new();
    compile_into(scheme, &mut out)?;
    out.push(Instr::Return);
    Ok(out)
}

/// Recursively append instructions for `scheme` into `out`.
///
/// After execution of the appended instructions, exactly one value remains on
/// the operand stack. No trailing [`Instr::Return`] is emitted — the caller
/// (`compile`) adds it once at the top level.
fn compile_into(scheme: &Scheme, out: &mut Vec<Instr>) -> Result<(), LangError> {
    match scheme {
        Scheme::Lit(v) => {
            out.push(Instr::Push(v.clone()));
        }
        Scheme::Var(name) => {
            if name == "$payload" {
                out.push(Instr::LoadPayloadVar);
            } else {
                out.push(Instr::Load(name.clone()));
            }
        }
        Scheme::Child(child_ref) => {
            out.push(Instr::EvalChild(child_ref.index()));
        }
        Scheme::Bind { name, value, body } => {
            compile_into(value, out)?;
            if name == "$payload" {
                out.push(Instr::StorePayloadName);
            } else {
                out.push(Instr::Store(name.clone()));
            }
            compile_into(body, out)?;
            out.push(Instr::PopScope);
        }
        Scheme::App { callee, args } => {
            compile_into(callee, out)?;
            for arg in args {
                compile_into(arg, out)?;
            }
            out.push(Instr::Call(args.len()));
        }
        Scheme::Branch {
            cond,
            then_,
            else_,
        } => {
            compile_into(cond, out)?;
            let jf_pos = out.len();
            out.push(Instr::JumpIfFalse(0)); // placeholder
            compile_into(then_, out)?;
            let j_pos = out.len();
            out.push(Instr::Jump(0)); // placeholder
            let else_target = out.len();
            out[jf_pos] = Instr::JumpIfFalse(else_target);
            compile_into(else_, out)?;
            let end = out.len();
            out[j_pos] = Instr::Jump(end);
        }
        Scheme::Prim { op, args } => {
            for arg in args {
                compile_into(arg, out)?;
            }
            out.push(Instr::Prim(op.clone(), args.len()));
        }
        Scheme::Foreign { name, args } => {
            for arg in args {
                compile_into(arg, out)?;
            }
            out.push(Instr::Foreign(name.clone(), args.len()));
        }
        Scheme::Vau { params, body } => {
            out.push(Instr::VauEnter);
            for (i, param) in params.iter().enumerate() {
                out.push(Instr::VauBind(param.clone(), i));
            }
            compile_into(body, out)?;
            out.push(Instr::PopScope);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{AstNode, AstRef, Value};
    use serde_json::json;

    /// A dummy node used by compilation tests (the node is not inspected).
    fn dummy_node() -> AstNode {
        AstNode::new("test")
    }

    // --- Combinator compilation tests ---

    #[test]
    fn compile_lit() {
        let instrs = compile(&Scheme::lit(json!(42)), &dummy_node()).unwrap();
        assert_eq!(instrs, vec![Instr::Push(json!(42)), Instr::Return]);
    }

    #[test]
    fn compile_lit_null() {
        let instrs = compile(&Scheme::lit(Value::Null), &dummy_node()).unwrap();
        assert_eq!(instrs, vec![Instr::Push(Value::Null), Instr::Return]);
    }

    #[test]
    fn compile_var_named() {
        let instrs = compile(&Scheme::var("x"), &dummy_node()).unwrap();
        assert_eq!(instrs, vec![Instr::Load("x".into()), Instr::Return]);
    }

    #[test]
    fn compile_var_payload() {
        let instrs = compile(&Scheme::var("$payload"), &dummy_node()).unwrap();
        assert_eq!(instrs, vec![Instr::LoadPayloadVar, Instr::Return]);
    }

    #[test]
    fn compile_child() {
        let instrs = compile(&Scheme::child(2), &dummy_node()).unwrap();
        assert_eq!(instrs, vec![Instr::EvalChild(2), Instr::Return]);
    }

    #[test]
    fn compile_bind_named() {
        let scheme = Scheme::Bind {
            name: "x".into(),
            value: Box::new(Scheme::lit(json!(1))),
            body: Box::new(Scheme::var("x")),
        };
        let instrs = compile(&scheme, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                Instr::Push(json!(1)),
                Instr::Store("x".into()),
                Instr::Load("x".into()),
                Instr::PopScope,
                Instr::Return,
            ]
        );
    }

    #[test]
    fn compile_bind_payload_name() {
        let scheme = Scheme::Bind {
            name: "$payload".into(),
            value: Box::new(Scheme::child(0)),
            body: Box::new(Scheme::child(1)),
        };
        let instrs = compile(&scheme, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                Instr::EvalChild(0),
                Instr::StorePayloadName,
                Instr::EvalChild(1),
                Instr::PopScope,
                Instr::Return,
            ]
        );
    }

    #[test]
    fn compile_app() {
        let scheme = Scheme::App {
            callee: Box::new(Scheme::var("f")),
            args: vec![Scheme::lit(json!(1)), Scheme::lit(json!(2))],
        };
        let instrs = compile(&scheme, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                Instr::Load("f".into()),
                Instr::Push(json!(1)),
                Instr::Push(json!(2)),
                Instr::Call(2),
                Instr::Return,
            ]
        );
    }

    #[test]
    fn compile_app_no_args() {
        let scheme = Scheme::App {
            callee: Box::new(Scheme::var("f")),
            args: vec![],
        };
        let instrs = compile(&scheme, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![Instr::Load("f".into()), Instr::Call(0), Instr::Return]
        );
    }

    #[test]
    fn compile_branch() {
        let scheme = Scheme::Branch {
            cond: Box::new(Scheme::var("c")),
            then_: Box::new(Scheme::lit(json!(1))),
            else_: Box::new(Scheme::lit(json!(2))),
        };
        let instrs = compile(&scheme, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                Instr::Load("c".into()), // 0: cond
                Instr::JumpIfFalse(4),    // 1: jump to else (index 4)
                Instr::Push(json!(1)),    // 2: then
                Instr::Jump(5),           // 3: jump to end (index 5)
                Instr::Push(json!(2)),    // 4: else
                Instr::Return,            // 5: end
            ]
        );
    }

    #[test]
    fn compile_prim() {
        let scheme = Scheme::prim(
            "add",
            vec![Scheme::child(0), Scheme::child(1)],
        );
        let instrs = compile(&scheme, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                Instr::EvalChild(0),
                Instr::EvalChild(1),
                Instr::Prim("add".into(), 2),
                Instr::Return,
            ]
        );
    }

    #[test]
    fn compile_prim_no_args() {
        let scheme = Scheme::prim("closure", vec![]);
        let instrs = compile(&scheme, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![Instr::Prim("closure".into(), 0), Instr::Return]
        );
    }

    #[test]
    fn compile_foreign() {
        let scheme = Scheme::foreign("double", vec![Scheme::child(0)]);
        let instrs = compile(&scheme, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                Instr::EvalChild(0),
                Instr::Foreign("double".into(), 1),
                Instr::Return,
            ]
        );
    }

    #[test]
    fn compile_nested_bind_in_branch() {
        // Branch with a Bind in the then-branch.
        let scheme = Scheme::Branch {
            cond: Box::new(Scheme::child(0)),
            then_: Box::new(Scheme::Bind {
                name: "x".into(),
                value: Box::new(Scheme::child(1)),
                body: Box::new(Scheme::var("x")),
            }),
            else_: Box::new(Scheme::lit(json!(0))),
        };
        let instrs = compile(&scheme, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                Instr::EvalChild(0),        // 0: cond
                Instr::JumpIfFalse(7),       // 1: jump to else (index 7)
                Instr::EvalChild(1),         // 2: value
                Instr::Store("x".into()),    // 3: store
                Instr::Load("x".into()),     // 4: body
                Instr::PopScope,             // 5: pop scope
                Instr::Jump(8),              // 6: jump to end (index 8)
                Instr::Push(json!(0)),       // 7: else
                Instr::Return,               // 8: end
            ]
        );
    }

    #[test]
    fn compile_deeply_nested() {
        // Prim("add", [Prim("mul", [child(0), child(1)]), child(2)])
        let scheme = Scheme::prim(
            "add",
            vec![
                Scheme::prim("mul", vec![Scheme::child(0), Scheme::child(1)]),
                Scheme::child(2),
            ],
        );
        let instrs = compile(&scheme, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                Instr::EvalChild(0),
                Instr::EvalChild(1),
                Instr::Prim("mul".into(), 2),
                Instr::EvalChild(2),
                Instr::Prim("add".into(), 2),
                Instr::Return,
            ]
        );
    }

    #[test]
    fn compile_child_uses_index() {
        // Verify that Child(AstRef) uses the .index() value.
        let scheme = Scheme::Child(AstRef::new(7));
        let instrs = compile(&scheme, &dummy_node()).unwrap();
        assert_eq!(instrs, vec![Instr::EvalChild(7), Instr::Return]);
    }

    #[test]
    fn compile_vau() {
        let scheme = Scheme::vau(
            vec!["a".into(), "b".into()],
            Scheme::lit(json!(42)),
        );
        let instrs = compile(&scheme, &dummy_node()).unwrap();
        assert_eq!(
            instrs,
            vec![
                Instr::VauEnter,
                Instr::VauBind("a".into(), 0),
                Instr::VauBind("b".into(), 1),
                Instr::Push(json!(42)),
                Instr::PopScope,
                Instr::Return,
            ]
        );
    }
}
