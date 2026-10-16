//! Regression: a **bare module-name import** resolves through the module
//! chain, so an ecosystem repo can ship a `.hnl` package whose imports *name*
//! modules instead of reaching across the filesystem.
//!
//! This is the seam the ADR-0128 item-7 / Phase 5 `.hnl` moves depend on: the
//! `web`/`-c` package cluster imports itself through relative paths
//! (`from "../generated/libc-c-linux-x86_64/socket" import { socket }`),
//! which forces a sibling-checkout layout across repos. Rewriting those specs
//! to bare names (`from "libc" import socket`) only works if the module chain
//! supplies the bindings — which is what this test locks down.
//!
//! Two mechanisms are exercised, both filing through `ModuleLookup`:
//!
//! - `ModuleLoader::strip_module_decl_lines` lowers a non-file-resolving
//!   `from "<name>" import <fn>` to `<fn> := "<name>.<fn>"` (ADR-0096); the
//!   string callee then dispatches through the module chain.
//! - the importing runtime's `ModuleLookup` holds the named module.
//!
//! The `libc` module here stands in for what `c/` (or `web/`) ships.

use std::path::PathBuf;
use std::rc::Rc;

use handl_language::{Source, Value};
use handl_platform::{EvalContext, EvalError, Function, Module, ModuleLookup};
use handl_plugin_engine_exec::ImportResolver;
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::DataLexerEngine;
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_plugin_module::ModuleRegistry;
use handl_runtime::{ModuleLoader, Runtime};
use serde_json::json;

/// `libc.socket(x)` — a stand-in host function that echoes its argument.
struct LibcSocket;

static LIBC_SOCKET: LibcSocket = LibcSocket;

impl Function for LibcSocket {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        Ok(args.first().cloned().unwrap_or(Value::Null))
    }
}

/// The `libc` module — one function, `socket`.
struct LibcModule;

impl Module for LibcModule {
    fn name(&self) -> &str {
        "libc"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "socket" => Some(&LIBC_SOCKET),
            _ => None,
        }
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        vec![("socket", &LIBC_SOCKET)]
    }
}

/// A unique temp directory for this test process's module fixtures.
fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hnl-bare-import-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Build a `ModuleLoader` + `Runtime` whose module chain contains `libc`.
fn loader_and_runtime(module_root: PathBuf) -> (Rc<ModuleLoader>, Runtime) {
    let kit = handl_kit_data_canonical::assemble().expect("canonical kit assembles");
    let lang_rc = Rc::new(kit.lang().clone());
    let modules: Rc<dyn ModuleLookup> = Rc::new(ModuleRegistry::new().with(LibcModule));

    let loader = Rc::new(ModuleLoader::new(
        Rc::clone(&lang_rc),
        Box::new(DataLexerEngine::new()),
        Box::new(PrattEngine::new()),
        Some(module_root),
    ));
    let mut loader_eval = TreeWalkEngine::new()
        .with_import_resolver(Rc::clone(&loader) as Rc<dyn ImportResolver>)
        .with_modules(Rc::clone(&modules));
    for (name, val) in &kit.lang().globals {
        loader_eval = loader_eval.with_global(name, val.clone());
    }
    loader.set_eval_engine(Box::new(loader_eval));

    let runtime = Runtime::new(kit.lang().clone())
        .with_lex_engine(Box::new(DataLexerEngine::new()))
        .with_parse_engine(PrattEngine::new())
        .with_eval_engine(Box::new(
            TreeWalkEngine::new()
                .with_import_resolver(Rc::clone(&loader) as Rc<dyn ImportResolver>)
                .with_modules(Rc::clone(&modules)),
        ))
        .with_import_resolver(Rc::clone(&loader) as Rc<dyn ImportResolver>)
        .with_modules(modules);
    (loader, runtime)
}

/// The `libc` module's `socket` is reachable from a module file that imports
/// it **by name** — the lowering runs, no file is looked for.
#[test]
fn bare_module_import_in_a_module_file_lowers_instead_of_failing() {
    let dir = scratch_dir("lower");
    std::fs::write(
        dir.join("consumer.hnl"),
        "from \"libc\" import socket\nexport use_it := (x) ~> socket(x)\n",
    )
    .expect("write module");

    let (loader, _rt) = loader_and_runtime(dir.clone());

    let value = loader
        .resolve_import("consumer.hnl")
        .expect("a bare-name import must lower, not fail file resolution");
    let Value::Object(obj) = &value else {
        panic!("module record should be an object, got {value}");
    };
    assert!(
        obj.contains_key("use_it"),
        "module should export `use_it`: {value}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// End to end: importing the module and applying its exported arrow reaches
/// `libc.socket` through the module chain.
#[test]
fn bare_module_import_dispatches_through_the_module_chain() {
    let dir = scratch_dir("dispatch");
    std::fs::write(
        dir.join("consumer.hnl"),
        "from \"libc\" import socket\nexport use_it := (x) ~> socket(x)\n",
    )
    .expect("write module");

    let (_loader, rt) = loader_and_runtime(dir.clone());

    let value = rt
        .run(&Source::new(
            r#"do { from "consumer.hnl" import use_it; use_it(7) }"#,
        ))
        .expect("import + apply should reach the `libc` module");

    assert_eq!(
        value,
        json!(7),
        "`socket` should dispatch through the `libc` module chain"
    );

    std::fs::remove_dir_all(&dir).ok();
}
