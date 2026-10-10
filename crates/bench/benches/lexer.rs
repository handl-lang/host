//! Lexer benchmarks.
//!
//! Measures canonical handl tokenization of a representative expression.
//! (The pre-refactor suite also measured a "streaming" mode via
//! `CanonicalLexer::lex_iter`, which the data-driven `LexerEngine` no longer
//! exposes — the batch path is the only surface.)

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use handl_bench::{canonical_lang, SAMPLE};
use handl_language::Source;
use handl_plugin_engine_lex::{LexerEngine, LexerRequest};
use handl_plugin_engine_lex_pattern::DataLexerEngine;

fn bench_lex_batch(c: &mut Criterion) {
    let lang = canonical_lang();
    let engine = DataLexerEngine::new();
    c.bench_function("lexer/batch", |b| {
        b.iter(|| {
            engine
                .lex(&lang, &LexerRequest::new(Source::new(black_box(SAMPLE))))
                .expect("sample lexes")
        });
    });
}

criterion_group!(lexer_benches, bench_lex_batch);
criterion_main!(lexer_benches);
