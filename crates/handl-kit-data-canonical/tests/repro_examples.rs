//! Temporary repro: lex+parse every example and report failures.

use handl_kit_data::{DataKit, DataKitManifest, FragmentSourceEntry};
use handl_kit_data_canonical;
use handl_language::{Language, Source};
use handl_plugin_engine_lex_pattern::DataLexerEngine;
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_runtime::{EngineRegistry, RuntimeConfig};
use std::path::{Path, PathBuf};

fn manifest() -> DataKitManifest {
    DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter("canonical-yaml", "canonical-yaml"))
        .with_runtime(RuntimeConfig::new().with_lex("data").with_parse("pratt"))
}

fn runtime() -> handl_runtime::Runtime {
    let reg = handl_kit_data_canonical::registry();
    let mut registry = EngineRegistry::new();
    registry.register_lex("data", Box::new(DataLexerEngine::new()));
    registry.register_parse("pratt", PrattEngine::new());
    let registry = std::rc::Rc::new(registry);
    let kit = DataKit::assemble(&manifest(), &reg).expect("canonical kit assembles");
    kit.into_runtime(&registry, None).expect("runtime binds")
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("walk: cannot read {}: {}", dir.display(), e);
            return;
        }
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().map_or(false, |e| e == "hnl") {
            out.push(p);
        }
    }
}

#[test]
fn repro_comment_close_value() {
    let lang = handl_kit_data_canonical::assemble().expect("kit assembles").lang().clone();
    for r in lang.lexer_rules.iter() {
        if r.id == "lex.comment" || r.id == "lex.doc_comment" {
            if let handl_language::LexerPattern::Delimited { open, close, .. } = &r.pattern {
                eprintln!("{} open={:?} close={:?}", r.id, open, close);
            }
        }
    }
}

#[test]
fn parse_all_examples() {
    let rt = runtime();
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let examples = crate_root.join("../../examples/handl");
    eprintln!("examples dir: {}", examples.display());
    let mut files = Vec::new();
    walk(&examples, &mut files);
    files.sort();
    let mut fail = 0;
    let mut ok = 0;
    for f in &files {
        let rel = f
            .strip_prefix(&crate_root)
            .unwrap_or(f)
            .display()
            .to_string();
        let src = std::fs::read_to_string(f).unwrap();
        let s = Source::new(&src);
        match rt.lex(&s) {
            Ok(toks) => match rt.parse(&toks) {
                Ok(_) => {
                    ok += 1;
                }
                Err(e) => {
                    fail += 1;
                    eprintln!("FAIL parse {}: {}", rel, e);
                }
            },
            Err(e) => {
                fail += 1;
                eprintln!("FAIL lex {}: {}", rel, e);
            }
        }
    }
    eprintln!("ok={} fail={} total={}", ok, fail, ok + fail);
    assert_eq!(fail, 0, "{} examples failed to parse", fail);
}
