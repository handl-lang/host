# bench (criterion micro-benchmarks)

Fine-grained criterion benchmarks for the language front end and pipeline.

This restores the `crates/bench` crate deleted in `bcf31afa` ("Remove obsolete
crates and files now superseded by the composable platform"). The original had
six benches; the refactor split the crates they targeted, so restoration is a
per-bench API mapping rather than a revert.

```bash
cargo bench -p handl-bench          # all restored benches
cargo bench -p handl-bench --bench lexer
```

## Status of the six original benches

| Original bench | Status | Notes |
| --- | --- | --- |
| `lexer` | ✅ restored | `CanonicalLexer::lex` → `DataLexerEngine::lex` (`plugin-engine-lex-pattern` + `plugin-engine-lex` `LexerRequest`). The old `lex_iter` streaming mode has no analogue. |
| `parser` | ✅ restored | `CanonicalParser::parse` → `PrattEngine::parse_dyn` (`plugin-engine-parse-pratt`). Both tokens-only and lex+parse variants. |
| `pipeline` | ⚠️ partly covered | The old bench drove `handl-compiler`'s `DriverPipeline`/`Session`, which no longer exists. The equivalent end-to-end parse+eval path is benchmarked by `kit-canonical/benches/perf.rs` (`parse_long_pipe`/`eval_long_pipe`). A dedicated stage-by-stage bench needs a decision on which seam (Runtime vs Kit) is the supported pipeline. |
| `typecheck` | ⏳ not restored | Targeted `handl-typecheck-core`/`-infer`/`-schema-json`; the modern entry point is `plugin-engine-typecheck::check_module(lang, src)`. Porting needs the bootstrap `Language` (the checker takes it from the caller — see `AGENTS.md` §"Cyclic dependency workaround"). |
| `infer` | ⏳ not restored | Targeted `handl-typecheck-infer::{infer, infer_cached, parse_expr, FunctionTable, ParseCache}`; that crate is gone. NBE/conversion checking may not have a direct analogue for `infer_cached`, so this needs a design decision, not a rename. |
| `core` | ⏳ not restored | Targeted `handl-typecheck-core::{SchemaInterpreter, Type, is_compatible}` + `schema_json`. Both crates are gone; the modern type universe is ADR-0106's object/sym graph. Needs a decision on what "type-compatibility micro-benchmark" means post-ADR-0106. |

## Why a separate crate

`kit-canonical/benches/perf.rs` benchmarks the *assembled kit* (parse+eval of a
long pipe). This crate benchmarks the *engines* directly, so a regression can
be attributed to the lexer or parser rather than to assembly or eval.
