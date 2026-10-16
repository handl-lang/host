//! Timing harness for the `HandlFunction::call` AST cache and the
//! `loader.hnl` self-hosting seam (ADR-0094).
//!
//! Run: `cargo run -p handl-plugin-lib-handl --example loader_bench --release`

use std::sync::Arc;
use std::time::Instant;

use handl_platform::{EvalContext, Module};
use handl_plugin_lib_char_native::CharModule;
use handl_plugin_lib_handl::{
    DEDENT_HNL, HandlLoader, ModuleResolver, build_str_handl_module, install_loader,
    strip_declaration_lines, transform_export_to_object,
};

struct CharResolver;

impl ModuleResolver for CharResolver {
    fn resolve(&self, name: &str) -> Option<Arc<dyn Module + Send + Sync>> {
        match name {
            "char" => Some(Arc::new(CharModule)),
            _ => None,
        }
    }
}

const STR_HANDL_HNL: &str = include_str!("../../../handl-packages/str-handl/str-handl.hnl");

/// Install the canonical default language for the bridge runtimes
/// (dev-dep data crate — examples may link `lang/` data).
fn ensure_lang() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(
            handl_kit_data_canonical::language_from_yaml()
                .expect("canonical language assembles"),
        );
    }
}

fn main() {
    ensure_lang();
    // --- Repeated HandlFunction::call on a .hnl module -------------------
    let str_mod = build_str_handl_module(&CharResolver);
    let f = str_mod.function("length").expect("str.length");
    let arg = serde_json::Value::String("x".repeat(1000));
    let mut ctx = EvalContext::new();

    let t = Instant::now();
    let _ = f.call(&[arg.clone()], &mut ctx).expect("warm call");
    println!("str.length first call (cold): {:>9?}", t.elapsed());

    let n = 20u32;
    let t = Instant::now();
    for _ in 0..n {
        let _ = f.call(&[arg.clone()], &mut ctx).expect("call");
    }
    let d = t.elapsed();
    println!("str.length {n} calls: {d:>9?} ({:?}/call)", d / n);

    // --- The loader path (bytecode-evaluated transforms) -----------------
    // Build the loader's str dep BEFORE installing (bootstrap paradox).
    // Both the loader and its `str` dep run on the bytecode engine
    // (`HandlLoader::new` selects bytecode internally; the str dep needs
    // the explicit opt-in here).
    let str_for_loader = build_str_handl_module(&CharResolver).with_eval_engine("bytecode");
    let _ = install_loader(HandlLoader::new(vec![
        Arc::new(CharModule),
        Arc::new(str_for_loader),
    ]));

    // Warm the loader's own AST (first call pays the one-time parse).
    let t = Instant::now();
    let out = strip_declaration_lines("x ~> x");
    println!(
        "loader strip_decls cold (tiny): {:>9?} -> {} chars",
        t.elapsed(),
        out.len()
    );

    let t = Instant::now();
    let out = strip_declaration_lines(DEDENT_HNL);
    println!(
        "loader strip_decls dedent.hnl ({} chars): {:>9?} -> {} chars",
        DEDENT_HNL.len(),
        t.elapsed(),
        out.len()
    );

    let t = Instant::now();
    let out = transform_export_to_object(DEDENT_HNL);
    println!(
        "loader transform_export dedent.hnl: {:>9?} -> {} chars",
        t.elapsed(),
        out.len()
    );

    let t = Instant::now();
    let out = strip_declaration_lines(STR_HANDL_HNL);
    println!(
        "loader strip_decls str-handl.hnl ({} chars): {:>9?} -> {} chars",
        STR_HANDL_HNL.len(),
        t.elapsed(),
        out.len()
    );
}
