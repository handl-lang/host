//! `handl-kit-lang` — the self-hosting meta chain.
//!
//! Extracted from `handl-kit-canonical`: the six `plugin-lib-lang-*` host
//! modules (`lex`/`parse`/`eval`/`typecheck`/`compile`/`run_compiled`)
//! wired as a delegate chain so a running handl program can call
//! `eval(lang, ast)`, `compile(lang, ast, target)`, etc.
//! (ADR-0041 Phase 2–3, ADR-0047 §1, ADR-0054).

use std::rc::Rc;

use handl_platform::ModuleLookup;
use handl_plugin_lib_lang_compile::CompileModule;
use handl_plugin_lib_lang_eval::EvalModule;
use handl_plugin_lib_lang_lex::LexModule;
use handl_plugin_lib_lang_parse::ParseModule;
use handl_plugin_lib_lang_run_compiled::RunCompiledModule;
use handl_plugin_lib_lang_typecheck::TypecheckModule;
use handl_runtime::RuntimeFactory;

/// Build the meta module chain
/// `lex → parse → eval → typecheck → compile → run_compiled → delegate`.
///
/// Each module holds `Rc<dyn RuntimeFactory>` (the `RuntimeFactoryImpl`)
/// and chains via delegate; `delegate` is the next lookup in the runtime's
/// module chain (`None` for a bare meta chain, e.g. `runtime_for_lang`).
/// The caller is responsible for `factory.set_meta_chain(chain)` —
/// `RuntimeFactoryImpl` breaks the chicken-and-egg after construction.
pub fn meta_chain(
    factory: &Rc<dyn RuntimeFactory>,
    delegate: Option<Rc<dyn ModuleLookup>>,
) -> Rc<dyn ModuleLookup> {
    let run_compiled_mod = Rc::new(RunCompiledModule::new(Rc::clone(factory), delegate));
    let compile_mod = Rc::new(CompileModule::new(
        Rc::clone(factory),
        Some(Rc::clone(&run_compiled_mod) as Rc<dyn ModuleLookup>),
    ));
    let typecheck_mod = Rc::new(TypecheckModule::new(
        Rc::clone(factory),
        Some(Rc::clone(&compile_mod) as Rc<dyn ModuleLookup>),
    ));
    let eval_mod = Rc::new(EvalModule::new(
        Rc::clone(factory),
        Some(Rc::clone(&typecheck_mod) as Rc<dyn ModuleLookup>),
    ));
    let parse_mod = Rc::new(ParseModule::new(
        Rc::clone(factory),
        Some(Rc::clone(&eval_mod) as Rc<dyn ModuleLookup>),
    ));
    Rc::new(LexModule::new(
        Rc::clone(factory),
        Some(Rc::clone(&parse_mod) as Rc<dyn ModuleLookup>),
    ))
}
