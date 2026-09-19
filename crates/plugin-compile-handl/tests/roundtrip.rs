//! Roundtrip tests for the Hughes arrow emitter.
//!
//! These tests lower the canonical `playground/*.hnl` files to `Term`, emit
//! them back to a handl-hughes source string, then parse *that* string with
//! the canonical parser and lower it again — the emitted text must produce
//! the original `Term`.
//!
//! The handl-hughes dialect *is* canonical handl surface syntax (ADR-0079
//! unified the arrow DSL onto the canonical language): `~>` lambdas,
//! `>>>`/`***`/`&&&` combinators, `arr <op>`, `{k: v}` objects and `expr.i`
//! field access. The parse side therefore goes through the same canonical
//! `lex`/`PrattEngine`/`lower_ast` pipeline the `.hnl` files do, so both
//! sides share one lowering contract.

use std::path::{Path, PathBuf};

use handl_kit_arrow_lower::lower_ast;
use handl_kit_data_canonical::language_from_yaml;
use handl_language::{Ast, AstRef, Language, Term};
use handl_plugin_compile_handl::HughesEmitter;
use handl_plugin_engine_lex_pattern::lex;
use handl_plugin_engine_parse::{ParserEngine, ParserRequest};
use handl_plugin_engine_parse_pratt::PrattEngine;

fn canonical_lang() -> Language {
    language_from_yaml().expect("canonical language should assemble")
}

fn parse_canonical(src: &str, lang: &Language) -> Ast {
    let tokens = lex(lang, src).expect("canonical lex");
    let engine = PrattEngine::new();
    let req = ParserRequest::new(tokens);
    engine.parse(lang, &req).expect("canonical parse")
}

fn lower_src(src: &str, lang: &Language) -> Term {
    let ast = parse_canonical(src, lang);
    let root = AstRef::new(ast.len() - 1);
    lower_ast(&ast, lang, root).expect("lower to term")
}

fn lower_file(path: &Path, lang: &Language) -> (Term, String) {
    let src = std::fs::read_to_string(path).expect("read playground file");
    let term = lower_src(&src, lang);
    let emitted = HughesEmitter::default().emit(&term);
    (term, emitted)
}

fn playground(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../playground")
        .join(name)
}

fn assert_roundtrip(name: &str) {
    let lang = canonical_lang();
    let (original, emitted) = lower_file(&playground(name), &lang);
    eprintln!("emitted:\n{}", emitted);
    eprintln!("original: {:?}", original);
    let parsed = lower_src(&emitted, &lang);
    eprintln!("parsed: {:?}", parsed);
    assert_eq!(parsed, original);
}

#[test]
fn roundtrip_composition_hughes() {
    assert_roundtrip("composition-hughes.hnl");
}

#[test]
fn roundtrip_composition_emidio() {
    assert_roundtrip("composition-emidio.hnl");
}
