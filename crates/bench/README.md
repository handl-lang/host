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
| `pipeline` | ✅ restored | The old bench drove `handl-compiler`'s `DriverPipeline`/`Session`, which no longer exists. Re-targeted onto `Kit` (the supported end-to-end seam): `pipeline/parse` and `pipeline/parse_and_eval`. Complementary to `kit-canonical/benches/perf.rs`, which measures a long `|>` chain — this measures the richer mixed-literal `SAMPLE`, so lex/parse cost dominates rather than composition depth. |
| `typecheck` | ✅ restored | Targeted `handl-typecheck-core`/`-infer`/`-schema-json`; re-targeted onto `plugin-engine-typecheck::check_module(&lang, src)`. The bench supplies the canonical language, which is what a host does (the checker takes its `Language` from the caller — see `AGENTS.md` §"Cyclic dependency workaround"). |
| `infer` | ⏳ not restorable as named | Targeted `handl-typecheck-infer::{infer, infer_cached, parse_expr, FunctionTable, ParseCache}`. That crate is gone and the NBE checker may have no direct analogue for `infer_cached`'s cache semantics, so this needs a design decision, not a rename. |
| `core` | ⏳ not restorable as named | Targeted `handl-typecheck-core::{SchemaInterpreter, Type, is_compatible}` + `schema_json`. Both crates are gone; the modern type universe is ADR-0106's object/sym graph. Needs a decision on what a "type-compatibility micro-benchmark" means post-ADR-0106. |

## Why a separate crate

`kit-canonical/benches/perf.rs` benchmarks the *assembled kit* (parse+eval of a
long pipe). This crate benchmarks the *engines* directly, so a regression can
be attributed to the lexer or parser rather than to assembly or eval.
