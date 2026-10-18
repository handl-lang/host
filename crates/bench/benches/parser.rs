//! Parser benchmarks.
//!
//! Measures the canonical Pratt parser on a representative expression, both
//! tokens-only (isolating parse cost) and lex+parse (the full front end).

use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use handl_bench::SAMPLE;

/// The canonical language, assembled from the embedded YAML fragments.
fn canonical_lang() -> handl_language::Language {
    handl_kit_data_canonical::language_from_yaml().expect("canonical language assembles")
}
use handl_language::Source;
use hnl_engine_lex::{LexerEngine, LexerRequest};
use hnl_engine_lex_pattern::DataLexerEngine;
use hnl_engine_parse::{ParserEngine, ParserRequest};
use hnl_engine_parse_pratt::PrattEngine;

fn bench_parse_only(c: &mut Criterion) {
    let lang = canonical_lang();
    let lexer = DataLexerEngine::new();
    let tokens = lexer
        .lex(&lang, &LexerRequest::new(Source::new(SAMPLE)))
        .expect("sample lexes");
    let parser = PrattEngine::new();
    c.bench_function("parser/parse_only", |b| {
        b.iter_batched(
            || tokens.clone(),
            |toks| {
                parser
                    .parse(&lang, &ParserRequest::new(toks))
                    .expect("sample parses")
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_lex_and_parse(c: &mut Criterion) {
    let lang = canonical_lang();
    let lexer = DataLexerEngine::new();
    let parser = PrattEngine::new();
    c.bench_function("parser/lex_and_parse", |b| {
        b.iter(|| {
            let tokens = lexer
                .lex(&lang, &LexerRequest::new(Source::new(black_box(SAMPLE))))
                .expect("sample lexes");
            parser
                .parse(&lang, &ParserRequest::new(tokens))
                .expect("sample parses")
        });
    });
}

criterion_group!(parser_benches, bench_parse_only, bench_lex_and_parse);
criterion_main!(parser_benches);
