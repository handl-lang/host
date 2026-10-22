//! Parity: the `.hnl`-authored desugar rewrites
//! (`handl-packages/desugar/desugar.hnl` behind `handl:desugar.*`) produce
//! structurally identical output to the Rust `desugar.*` natives.

use std::sync::Arc;

use handl_language::{Ast, AstRef, Language, Source};
use hnl_engine_lex::{LexerEngine, LexerRequest};
use hnl_engine_lex_pattern::DataLexerEngine;
use hnl_engine_parse::ParserRequest;
use handl_runtime::ParseEngineDyn;
use hnl_engine_parse_pratt::PrattEngine;
use hnl_engine_transform_rewrite::native::desugar_native_registry;
use hnl_engine_transform_rewrite::RewriteEngine;
use handl_plugin_lib_char_native::CharModule;
use handl_plugin_lib_handl::transform::DESUGAR_HNL;
use handl_plugin_lib_handl::{HandlModule, HandlNativeRewrite};
use serde_json::Value;

fn lang() -> &'static Language {
    static LANG: std::sync::OnceLock<Language> = std::sync::OnceLock::new();
    LANG.get_or_init(|| {
        let l = handl_kit_data_prelude::language_from_yaml()
            .expect("canonical language");
        let _ = handl_plugin_lib_handl::install_default_language(l.clone());
        l
    })
}

/// Raw-parse `src` (no desugar) with the canonical lexer + Pratt parser.
fn parse_raw(src: &str) -> Ast {
    let lang = lang();
    let toks = DataLexerEngine::new()
        .lex(lang, &LexerRequest::new(Source::new(src)))
        .expect("lex");
    PrattEngine::new()
        .parse_dyn(lang, &ParserRequest::new(toks))
        .expect("parse")
}

fn desugar_hnl_module() -> HandlModule {
    HandlModule::new("desugar", DESUGAR_HNL, &["data", "match_recursor"])
        .with_dependency_arc(Arc::new(CharModule))
        .with_dependency_arc(Arc::new(handl_runtime::vm_primitives::SymModule::new()))
        .with_eval_engine("tree-walk")
}

/// Desugar via the `.hnl` overlay.
fn handl_desugar(ast: &Ast) -> Ast {
    let natives = HandlNativeRewrite::new()
        .with_module(desugar_hnl_module())
        .with_fallback(Arc::new(desugar_native_registry()));
    let engine = RewriteEngine::new().with_desugar_and_handl_natives(Arc::new(natives));
    let root = AstRef::new(ast.len() - 1);
    engine
        .run_pass(lang(), ast, root, "desugar")
        .expect("handl desugar")
        .ast
}

/// Desugar via the Rust natives only — the registry dual-registers the
/// `handl:desugar.*` spellings, so the same fallback table resolves them.
fn rust_desugar(ast: &Ast) -> Ast {
    let engine = RewriteEngine::new()
        .with_desugar_and_handl_natives(Arc::new(desugar_native_registry()));
    let root = AstRef::new(ast.len() - 1);
    engine
        .run_pass(lang(), ast, root, "desugar")
        .expect("rust desugar")
        .ast
}

/// Structural equality ignoring span; numbers normalize to f64 (handl
/// emits floats) and `children`/`body` entries are compared as nodes.
fn same_node(a: &Ast, ai: usize, b: &Ast, bi: usize, depth: usize) -> bool {
    if depth > 10000 {
        return false;
    }
    let (x, y) = match (a.get(AstRef::new(ai)), b.get(AstRef::new(bi))) {
        (Some(x), Some(y)) => (x, y),
        _ => return false,
    };
    if x.kind != y.kind {
        eprintln!("kind mismatch {ai}/{bi}: {:?} vs {:?}", x.kind, y.kind);
        return false;
    }
    same_payload(a, &x.payload, b, &y.payload, depth)
}

fn norm(v: &Value) -> Value {
    match v {
        Value::Number(n) => Value::from(n.as_f64().unwrap_or(0.0)),
        _ => v.clone(),
    }
}

static PATH: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

fn same_payload(a: &Ast, pa: &Value, b: &Ast, pb: &Value, depth: usize) -> bool {
    match (pa, pb) {
        (Value::Object(ma), Value::Object(mb)) => {
            let mut ka: Vec<_> = ma.keys().collect();
            let mut kb: Vec<_> = mb.keys().collect();
            ka.sort();
            kb.sort();
            if ka != kb {
                eprintln!("payload keys differ: {ka:?} vs {kb:?} path={:?}", PATH.lock().unwrap());
                return false;
            }
            for k in ka {
                if k == "children" {
                    let (xs, ys) = match (
                        ma[k].as_array(),
                        mb[k].as_array(),
                    ) {
                        (Some(x), Some(y)) => (x, y),
                        _ => return false,
                    };
                    if xs.len() != ys.len() {
                        eprintln!("children len differ");
                        return false;
                    }
                    for (xv, yv) in xs.iter().zip(ys) {
                        match (xv.as_u64(), yv.as_u64()) {
                            (Some(xi), Some(yi)) => {
                                if !same_node(a, xi as usize, b, yi as usize, depth + 1)
                                {
                                    return false;
                                }
                            }
                            _ => return false,
                        }
                    }
                } else if k == "body" {
                    match (ma[k].as_u64(), mb[k].as_u64()) {
                        (Some(xi), Some(yi)) => {
                            PATH.lock().unwrap().push("body".to_string());
                            let ok = same_node(a, xi as usize, b, yi as usize, depth + 1);
                            PATH.lock().unwrap().pop();
                            if !ok {
                                return false;
                            }
                        }
                        _ => return false,
                    }
                } else if norm(&ma[k]) != norm(&mb[k]) {
                    eprintln!("payload[{k}] differs: {:?} vs {:?}", ma[k], mb[k]);
                    return false;
                }
            }
            true
        }
        _ => norm(pa) == norm(pb),
    }
}

fn dump(ast: &Ast, path: &str) {
    let nodes: Vec<Value> = (0..ast.len())
        .map(|i| {
            let n = ast.get(AstRef::new(i)).unwrap();
            serde_json::json!({"i": i, "kind": n.kind, "payload": n.payload})
        })
        .collect();
    std::fs::write(path, serde_json::to_string_pretty(&nodes).unwrap()).unwrap();
}

fn assert_parity(src: &str) {
    let ast = parse_raw(src);
    let h = handl_desugar(&ast);
    let r = rust_desugar(&ast);
    if std::env::var_os("DUMP").is_some() {
        dump(&h, "/tmp/h.json");
        dump(&r, "/tmp/r.json");
    }
    let (hr, rr) = (h.len() - 1, r.len() - 1);
    assert!(
        same_node(&h, hr, &r, rr, 0),
        "desugar output diverged for {src:?}"
    );
}

#[test]
fn data_nat_parity() {
    assert_parity("data Nat :: Zero | Succ(pred : Nat)");
}

#[test]
fn data_bool_parity() {
    assert_parity("data Bool :: True | False");
}

#[test]
fn data_three_ctors_parity() {
    assert_parity("data RGB :: R | G | B");
}

#[test]
fn data_maybe_parity() {
    assert_parity("data Maybe :: Nothing | Just(x : Nat)");
}

#[test]
fn match_nat_parity() {
    assert_parity(
        "data Nat :: Zero | Succ(pred : Nat)\nx :: match n { Zero: 0, Succ(p): p }",
    );
}

#[test]
fn match_conditional_declines_parity() {
    assert_parity("x :: match n { \"a\": 1, _: 0 }");
}
