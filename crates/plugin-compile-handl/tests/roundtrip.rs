//! Roundtrip tests for the Hughes arrow emitter.
//!
//! These tests lower the canonical `playground/*.hnl` files to `Term`, emit
//! them back to a Hughes-style source string, then parse that string with
//! the arrow parser and compare the resulting `Term` to the original.

use std::path::{Path, PathBuf};

use handl_language::{Ast, AstRef, Language, Term};
use handl_kit_arrow_lower::lower_ast;
use handl_kit_data_canonical::language_from_yaml;
use handl_plugin_compile_handl::HughesEmitter;
use handl_plugin_engine_lex_pattern::lex;
use handl_plugin_engine_parse::{ParserEngine, ParserRequest};
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_plugin_ext_refinement::{parse_arrow_file, ParsedFile};

fn canonical_lang() -> Language {
    language_from_yaml().expect("canonical language should assemble")
}

fn parse_canonical(src: &str, lang: &Language) -> Ast {
    let tokens = lex(lang, src).expect("canonical lex");
    let engine = PrattEngine::new();
    let req = ParserRequest::new(tokens);
    engine.parse(lang, &req).expect("canonical parse")
}

fn lower_file(path: &Path, lang: &Language) -> (Term, String) {
    let src = std::fs::read_to_string(path).expect("read playground file");
    let ast = parse_canonical(&src, lang);
    let root = AstRef::new(ast.len() - 1);
    let term = lower_ast(&ast, lang, root).expect("lower to term");
    let emitted = HughesEmitter::default().emit(&term);
    (term, emitted)
}

fn playground(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../playground")
        .join(name)
}

#[test]
fn roundtrip_composition_hughes() {
    let lang = canonical_lang();
    let (original, emitted) = lower_file(&playground("composition-hughes.hnl"), &lang);
    eprintln!("emitted:\n{}", emitted);
    eprintln!("original: {:?}", original);
    match parse_arrow_file(&emitted) {
        Ok(parsed) => {
            eprintln!("parsed: {:?}", parsed);
            if let ParsedFile::Single(parsed_term) = parsed {
                assert_eq!(parsed_term, original);
            } else {
                panic!("expected a single expression, got named arrows");
            }
        }
        Err(e) => panic!("parse_arrow_file failed: {}", e),
    }
}

#[test]
fn roundtrip_composition_emidio() {
    let lang = canonical_lang();
    let (original, emitted) = lower_file(&playground("composition-emidio.hnl"), &lang);
    eprintln!("emitted:\n{}", emitted);
    eprintln!("original: {:?}", original);
    match parse_arrow_file(&emitted) {
        Ok(parsed) => {
            eprintln!("parsed: {:?}", parsed);
            if let ParsedFile::Single(parsed_term) = parsed {
                assert_eq!(parsed_term, original);
            } else {
                panic!("expected a single expression, got named arrows");
            }
        }
        Err(e) => panic!("parse_arrow_file failed: {}", e),
    }
}
