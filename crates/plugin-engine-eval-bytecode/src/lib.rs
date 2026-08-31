//! # handl-plugin-engine-eval-bytecode
//!
//! The **bytecode eval engine** for the eval operation (ADR-0044 §4, §7).
//!
//! This engine compiles [`handl_language::Scheme`] data to a bytecode
//! representation and executes it via a stack-based VM. It shares the same
//! `Scheme` vocabulary as the
//! [`handl_plugin_engine_eval_tree_walk`](handl_plugin_engine_eval_tree_walk)
//! reference interpreter; conformance tests verify both produce **identical
//! results**.
//!
//! ## Compilers erase data dispatch (ADR-0044 §7)
//!
//! The tree-walk pays for per-node scheme lookup (`lang.eval[node.kind]` →
//! `Scheme`) on every execution. The bytecode engine **compiles this lookup
//! away**: at compile time, every node kind's scheme is resolved to a fixed
//! instruction sequence. The compiled artifact has no hash lookups, no string
//! matching, no `lang.eval` access at execution time.
//!
//! A per-node compilation cache (compile each node's scheme once, cache by
//! [`AstRef`](handl_language::AstRef)) is used to demonstrate this. When
//! [`Instr::EvalChild`] runs, it looks up the child's `AstRef` in the cache;
//! on a miss, it compiles the child's scheme (one-time `lang.eval` lookup)
//! and caches the result. On subsequent executions, the cached instructions
//! are reused — no data dispatch occurs. Full ahead-of-time compilation of
//! the whole AST is a future refinement.
//!
//! ## Bytecode ISA
//!
//! See the [`instr`] module for the full instruction set documentation. The
//! ISA covers all 8 `Scheme` combinators:
//!
//! | Scheme combinator | Compiled instructions |
//! |---|---|
//! | `Lit(v)` | `Push(v)` |
//! | `Var("x")` | `Load("x")` |
//! | `Var("$payload")` | `LoadPayloadVar` |
//! | `Child(n)` | `EvalChild(n)` |
//! | `Bind{name, value, body}` | `compile(value)`, `Store(name)`, `compile(body)`, `PopScope` |
//! | `App{callee, args}` | `compile(callee)`, `compile(args)…`, `Call(n)` |
//! | `Branch{cond, then, else}` | `compile(cond)`, `JumpIfFalse(else)`, `compile(then)`, `Jump(end)`, `compile(else)` |
//! | `Prim{op, args}` | `compile(args)…`, `Prim(op, n)` |
//! | `Foreign{name, args}` | `compile(args)…`, `Foreign(name, n)` |
//! | `Vau{params, body}` | `VauEnter`, `VauBind(p_i, i)…`, `compile(body)`, `PopScope` |
//!
//! ## Conventions (matched to the tree-walk)
//!
//! - `Child(n)` = nth child of current node: reads `node.payload["children"][n]`
//!   to get the absolute `AstRef`, then recurses.
//! - `Var("$payload")` reads `node.payload` as the variable name.
//! - `Bind{name: "$payload", ...}` reads `node.payload["name"]` as the bind name.
//! - No-scheme fallback: if a node kind has no eval scheme, returns
//!   `node.payload` directly.
//! - Closure representation: `Value::Object` with
//!   `{"__closure__": true, "params": [...], "body": <arena_idx>, "env": {...}}`,
//!   created by the `"closure"` primitive.
//! - Primitives: `add, sub, mul, div, eq, lt, gt, not, and, or, closure` —
//!   identical semantics to the tree-walk.
//! - Scope: `Vec<HashMap<String, Value>>` scope stack; `Bind` pushes a child
//!   scope; lookup searches innermost-first. Closure calls swap in the
//!   captured env, bind params, eval body, restore.

#![warn(missing_docs)]

pub mod compile;
pub mod instr;
mod vm;

pub use compile::compile;
pub use instr::Instr;

use handl_language::{Ast, AstRef, LangError, Language, SchemeVersion, Value};
use handl_plugin_engine_eval::{EvalEngine, EvalRequest};
use std::collections::HashMap;
use std::rc::Rc;

use vm::VmFrame;

/// The bytecode eval engine.
///
/// Compiles `Scheme` data to bytecode and executes via a stack-based VM.
/// Construct with [`BytecodeEngine::new`] and optionally pass a module
/// registry via [`BytecodeEngine::with_modules`].
///
/// The engine checks `lang.scheme_version` against `SchemeVersion::v1()` and
/// rejects mismatched versions.
#[derive(Clone)]
pub struct BytecodeEngine {
    /// The underlying VM (carries the module registry).
    vm: BytecodeVm,
    /// Global variables injected by the host (e.g. `self` → language value).
    globals: HashMap<String, Value>,
}

impl BytecodeEngine {
    /// Create a new bytecode engine with no module registry.
    pub fn new() -> Self {
        Self {
            vm: BytecodeVm::new(),
            globals: HashMap::new(),
        }
    }

    /// Create a bytecode engine with a module registry attached.
    ///
    /// The module registry is used for [`handl_language::Scheme::Foreign`]
    /// dispatch (ADR-0044 §7).
    pub fn with_modules(modules: Rc<dyn handl_platform::ModuleLookup>) -> Self {
        Self {
            vm: BytecodeVm::with_modules(modules),
            globals: HashMap::new(),
        }
    }

    /// Inject a global variable into the root scope (builder style).
    /// ADR-0041 Phase 3: `self` is an ambient variable, not a function call.
    pub fn with_global(mut self, name: &str, value: Value) -> Self {
        self.globals.insert(name.to_string(), value);
        self
    }
}

impl Default for BytecodeEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl EvalEngine for BytecodeEngine {
    fn eval(&self, lang: &Language, req: &EvalRequest) -> Result<Value, LangError> {
        let ast = &req.ast;
        if ast.is_empty() {
            return Ok(Value::Null);
        }
        // Scheme version check — reject versions the engine doesn't support.
        if !SchemeVersion::v1().supports(&lang.scheme_version) {
            return Err(LangError::Engine(format!(
                "unsupported scheme version: {} (engine supports v1)",
                lang.scheme_version
            )));
        }
        let root_ref = AstRef::new(ast.len() - 1);
        let root_node = ast.at(root_ref)?;
        // Compile the root node's scheme (or use the no-scheme fallback).
        let scheme_def = lang.eval.get_by_identity(&root_node.kind);
        let instrs = match scheme_def {
            Some(def) => compile(&def.scheme, root_node)?,
            None => vec![Instr::LoadPayload, Instr::Return],
        };
        self.vm.execute(lang, ast, &instrs, &self.globals)
    }

    /// Produce an owned boxed clone (ADR-0047 §2).
    fn box_clone(&self) -> Box<dyn EvalEngine> {
        Box::new(self.clone())
    }

    /// Produce a fresh owned engine with the given module registry
    /// wired in, preserving any previously-injected globals (ADR-0047 §2).
    fn with_modules(&self, modules: Rc<dyn handl_platform::ModuleLookup>) -> Box<dyn EvalEngine> {
        let mut engine = self.clone();
        engine.vm = BytecodeVm::with_modules(modules);
        Box::new(engine)
    }

    /// Produce a fresh owned engine with an additional global variable
    /// injected into the root scope (ADR-0047 §2).
    fn with_global(&self, name: &str, val: Value) -> Box<dyn EvalEngine> {
        Box::new(self.clone().with_global(name, val))
    }
}

/// The bytecode VM.
///
/// Executes a compiled instruction program against an operand stack and a
/// scope stack. [`Instr::EvalChild`] and [`Instr::Call`] recurse into
/// per-node compilation (via a cache) and execution.
///
/// Construct with [`BytecodeVm::new`] and optionally pass a module
/// registry via [`BytecodeVm::with_modules`].
#[derive(Clone)]
pub struct BytecodeVm {
    /// The module registry for `Foreign` dispatch (may be `None`).
    modules: Option<Rc<dyn handl_platform::ModuleLookup>>,
}

impl BytecodeVm {
    /// Create a new VM with no module registry.
    pub fn new() -> Self {
        Self { modules: None }
    }

    /// Create a new VM with a module registry.
    pub fn with_modules(modules: Rc<dyn handl_platform::ModuleLookup>) -> Self {
        Self {
            modules: Some(modules),
        }
    }

    /// Execute a pre-compiled instruction program.
    ///
    /// The program is executed against the root node (the last node in the
    /// arena, following the Pratt convention). [`Instr::EvalChild`]
    /// instructions compile and execute child nodes on demand via a per-node
    /// cache.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on evaluation failure (unknown variable,
    /// unknown primitive, unregistered foreign function, stack underflow,
    /// etc.).
    pub fn execute(
        &self,
        lang: &Language,
        ast: &Ast,
        instrs: &[Instr],
        globals: &HashMap<String, Value>,
    ) -> Result<Value, LangError> {
        if ast.is_empty() {
            return Ok(Value::Null);
        }
        let root_ref = AstRef::new(ast.len() - 1);
        let root_node = ast.at(root_ref)?;
        let mut frame = VmFrame::new(ast, lang, self.modules.as_ref());
        for (name, val) in globals {
            frame.define_global(name, val.clone());
        }
        frame.exec_instrs(instrs, root_node)
    }
}

impl Default for BytecodeVm {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CompileEngine / CompiledArtifact impls (ADR-0047 §3)
// ---------------------------------------------------------------------------

/// A compiled bytecode artifact. Holds the root node's compiled instruction
/// program plus the [`Language`] and [`Ast`] it was compiled from (needed
/// because [`Instr::EvalChild`] lazily compiles child nodes using `lang` +
/// `ast` at execution time via the per-node cache).
#[derive(Clone)]
pub struct BytecodeArtifact {
    /// The language the program was compiled under.
    lang: Language,
    /// The AST the program was compiled from.
    ast: Ast,
    /// The compiled instruction program for the root node.
    root_instrs: Vec<Instr>,
}

impl handl_runtime::CompiledArtifact for BytecodeArtifact {
    fn run(
        &self,
        modules: Option<Rc<dyn handl_platform::ModuleLookup>>,
    ) -> Result<Value, LangError> {
        let vm = match modules {
            Some(m) => BytecodeVm::with_modules(m),
            None => BytecodeVm::new(),
        };
        let globals = HashMap::new();
        vm.execute(&self.lang, &self.ast, &self.root_instrs, &globals)
    }

    fn box_clone(&self) -> Box<dyn handl_runtime::CompiledArtifact> {
        Box::new(self.clone())
    }
}

/// The bytecode compile engine. Produces [`BytecodeArtifact`] impls from an
/// [`Ast`] (ADR-0047 §3). Registered under the target name `"bytecode"`.
#[derive(Clone, Default)]
pub struct BytecodeCompileEngine;

impl BytecodeCompileEngine {
    /// Create a new bytecode compile engine.
    pub fn new() -> Self {
        Self
    }
}

impl handl_runtime::CompileEngine for BytecodeCompileEngine {
    fn compile(
        &self,
        lang: &Language,
        ast: &Ast,
        _target: &str,
    ) -> Result<Box<dyn handl_runtime::CompiledArtifact>, LangError> {
        if ast.is_empty() {
            return Ok(Box::new(BytecodeArtifact {
                lang: lang.clone(),
                ast: ast.clone(),
                root_instrs: Vec::new(),
            }));
        }
        if !SchemeVersion::v1().supports(&lang.scheme_version) {
            return Err(LangError::Engine(format!(
                "compile: unsupported scheme version: {} (engine supports v1)",
                lang.scheme_version
            )));
        }
        let root_ref = AstRef::new(ast.len() - 1);
        let root_node = ast.at(root_ref)?;
        // Compile the root node's scheme (or use the no-scheme fallback).
        // Child nodes are compiled lazily at execution time via the VM's
        // per-node cache.
        let scheme_def = lang.eval.get_by_identity(&root_node.kind);
        let root_instrs = match scheme_def {
            Some(def) => compile(&def.scheme, root_node)?,
            None => vec![Instr::LoadPayload, Instr::Return],
        };
        Ok(Box::new(BytecodeArtifact {
            lang: lang.clone(),
            ast: ast.clone(),
            root_instrs,
        }))
    }

    fn box_clone(&self) -> Box<dyn handl_runtime::CompileEngine> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{AstNode, EvalSchemeDef, Language, Scheme, Value};
    use handl_platform::{EvalContext, Function, Module, ModuleLookup};
    use handl_plugin_engine_eval::EvalRequest;
    use handl_plugin_module::ModuleRegistry;
    use serde_json::json;

    /// Build a simple AST for testing.
    fn make_ast(nodes: Vec<AstNode>) -> handl_language::Ast {
        let mut ast = handl_language::Ast::new();
        for n in nodes {
            ast.push(n);
        }
        ast
    }

    /// A host function that doubles its integer argument.
    struct DoubleHostFn;

    impl Function for DoubleHostFn {
        fn call(
            &self,
            args: &[Value],
            _ctx: &mut EvalContext,
        ) -> Result<Value, handl_platform::EvalError> {
            let n = args[0].as_i64().unwrap();
            Ok(Value::from(n * 2))
        }
    }

    /// A module with "double" registered.
    struct ConformanceModule;

    impl Module for ConformanceModule {
        fn name(&self) -> &str {
            "conformance"
        }
        fn function(&self, name: &str) -> Option<&dyn Function> {
            if name == "double" {
                Some(&DoubleHostFn)
            } else {
                None
            }
        }
    }

    /// Build a module registry with the `double` function registered.
    fn conformance_registry() -> Rc<dyn ModuleLookup> {
        Rc::new(ModuleRegistry::new().with(ConformanceModule))
    }

    // --- BytecodeEngine tests ---

    #[test]
    fn engine_empty_ast_returns_null() {
        let engine = BytecodeEngine::new();
        let lang = Language::new();
        let req = EvalRequest::new(handl_language::Ast::new());
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn engine_no_scheme_returns_payload() {
        let engine = BytecodeEngine::new();
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("lit").with_payload(json!(42))]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn engine_lit_scheme() {
        let engine = BytecodeEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.const",
                "const",
                Scheme::lit(json!(99)),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("const")]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(99));
    }

    #[test]
    fn engine_add() {
        let engine = BytecodeEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("lit").with_payload(json!(3)),
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(5));
    }

    #[test]
    fn engine_let_binding() {
        let engine = BytecodeEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::var("$payload")))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                Scheme::Bind {
                    name: "$payload".into(),
                    value: Box::new(Scheme::child(0)),
                    body: Box::new(Scheme::child(1)),
                },
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(42)),
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(42));
    }

    #[test]
    fn engine_if_true() {
        let engine = BytecodeEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.if",
                "if",
                Scheme::Branch {
                    cond: Box::new(Scheme::child(0)),
                    then_: Box::new(Scheme::child(1)),
                    else_: Box::new(Scheme::child(2)),
                },
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(true)),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(1));
    }

    #[test]
    fn engine_if_false() {
        let engine = BytecodeEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.if",
                "if",
                Scheme::Branch {
                    cond: Box::new(Scheme::child(0)),
                    then_: Box::new(Scheme::child(1)),
                    else_: Box::new(Scheme::child(2)),
                },
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(false)),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(2));
    }

    #[test]
    fn engine_foreign() {
        let engine = BytecodeEngine::with_modules(conformance_registry());
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.dbl",
                "dbl",
                Scheme::foreign("double", vec![Scheme::child(0)]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(21)),
            AstNode::new("dbl").with_payload(json!({"children": [0]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(42));
    }

    #[test]
    fn engine_foreign_no_host_errors() {
        let engine = BytecodeEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.dbl",
                "dbl",
                Scheme::foreign("double", vec![Scheme::child(0)]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(21)),
            AstNode::new("dbl").with_payload(json!({"children": [0]})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn engine_nested_let_add() {
        let engine = BytecodeEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::var("$payload")))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                Scheme::Bind {
                    name: "$payload".into(),
                    value: Box::new(Scheme::child(0)),
                    body: Box::new(Scheme::child(1)),
                },
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(3)),
            AstNode::new("lit").with_payload(json!(4)),
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("var").with_payload(json!("y")),
            AstNode::new("binary.add").with_payload(json!({"children": [2, 3]})),
            AstNode::new("let").with_payload(json!({"name": "y", "children": [1, 4]})),
            AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 5]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(7));
    }

    #[test]
    fn engine_version_mismatch_errors() {
        let engine = BytecodeEngine::new();
        let mut lang = Language::new();
        lang.scheme_version = SchemeVersion::new("v2");
        let ast = make_ast(vec![AstNode::new("lit").with_payload(json!(42))]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("unsupported scheme version"));
    }

    #[test]
    fn engine_default_constructor() {
        let engine = BytecodeEngine::default();
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("lit").with_payload(json!(0))]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(0));
    }

    #[test]
    fn engine_trait_dispatch_via_dyn() {
        let engine: Box<dyn EvalEngine> = Box::new(BytecodeEngine::new());
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("lit").with_payload(json!(99))]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(99));
    }

    #[test]
    fn engine_closure_call() {
        let engine = BytecodeEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.fn",
                "function",
                Scheme::prim("closure", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::var("$payload")))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.call",
                "call",
                Scheme::App {
                    callee: Box::new(Scheme::child(0)),
                    args: vec![Scheme::child(1)],
                },
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
            AstNode::new("function").with_payload(json!({"params": ["x"], "body": 2})),
            AstNode::new("lit").with_payload(json!(5)),
            AstNode::new("call").with_payload(json!({"children": [3, 4]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(6));
    }

    #[test]
    fn engine_closure_captures_env() {
        let engine = BytecodeEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::var("$payload")))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                Scheme::Bind {
                    name: "$payload".into(),
                    value: Box::new(Scheme::child(0)),
                    body: Box::new(Scheme::child(1)),
                },
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.fn",
                "function",
                Scheme::prim("closure", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.call",
                "call",
                Scheme::App {
                    callee: Box::new(Scheme::child(0)),
                    args: vec![Scheme::child(1)],
                },
            ))
            .unwrap();
        // let x = 10 in let f = fn(y) { x + y } in f(5) → 15
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(10)),
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("var").with_payload(json!("y")),
            AstNode::new("binary.add").with_payload(json!({"children": [1, 2]})),
            AstNode::new("function").with_payload(json!({"params": ["y"], "body": 3})),
            AstNode::new("lit").with_payload(json!(5)),
            AstNode::new("call").with_payload(json!({"children": [4, 5]})),
            AstNode::new("let").with_payload(json!({"name": "f", "children": [4, 6]})),
            AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 7]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(15));
    }

    // --- BytecodeVm tests ---

    #[test]
    fn vm_default_constructor() {
        let vm = BytecodeVm::default();
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![Instr::Push(json!(42)), Instr::Return];
        let result = vm.execute(&lang, &ast, &instrs, &HashMap::new()).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn vm_empty_ast_returns_null() {
        let vm = BytecodeVm::new();
        let lang = Language::new();
        let instrs = vec![Instr::Return];
        let result = vm
            .execute(&lang, &handl_language::Ast::new(), &instrs, &HashMap::new())
            .unwrap();
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn vm_execute_precompiled() {
        let vm = BytecodeVm::new();
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(10)),
            Instr::Push(json!(20)),
            Instr::Prim("add".into(), 2),
            Instr::Return,
        ];
        let result = vm.execute(&lang, &ast, &instrs, &HashMap::new()).unwrap();
        assert_eq!(result, json!(30));
    }

    #[test]
    fn engine_foreign_host_present_but_fn_not_found_errors() {
        // Module registry is registered but doesn't have the requested function.
        let engine = BytecodeEngine::with_modules(conformance_registry());
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.triple",
                "triple",
                Scheme::foreign("triple", vec![Scheme::child(0)]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(21)),
            AstNode::new("triple").with_payload(json!({"children": [0]})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("unregistered foreign"));
    }

    // --- Vau (fexpr) tests (ADR-0041 §6, ADR-0045) ---

    #[test]
    fn engine_vau_does_not_force_child() {
        // A vau node binds its child (unevaluated) but the body returns a
        // constant. The child is a div-by-zero node that would error if
        // evaluated, proving the child is never forced.
        let engine = BytecodeEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.div",
                "binary.div",
                Scheme::prim("div", vec![Scheme::child(0), Scheme::child(1)]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.vau",
                "vau",
                Scheme::vau(vec!["x".into()], Scheme::lit(json!(42))),
            ))
            .unwrap();
        // 0: lit(1), 1: lit(0), 2: binary.div {children:[0,1]} — would error
        // 3: vau {children:[2]} — binds x to the unevaluated div, returns 42
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!(0)),
            AstNode::new("binary.div").with_payload(json!({"children": [0, 1]})),
            AstNode::new("vau").with_payload(json!({"children": [2]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(42));
    }
}
