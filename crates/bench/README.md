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
| `pipeline` | ✅ restored | The old bench drove `handl-compiler`'s `DriverPipeline`/`Session`, which no longer exists. Re-targeted onto `Kit` (the supported end-to-end seam): `pipeline/parse` and `pipeline/parse_and_eval`, measuring the richer mixed-literal `SAMPLE`, so lex/parse cost dominates rather than composition depth. |
| `typecheck` | ✅ restored | Targeted `handl-typecheck-core`/`-infer`/`-schema-json`; re-targeted onto `plugin-engine-typecheck::check_module(&lang, src)`, the bootstrap checker. The bench supplies the canonical language, which is what a host does (the checker takes its `Language` from the caller — see `AGENTS.md` §"Cyclic dependency workaround"). |
| `infer` | ✅ re-targeted | Targeted `handl-typecheck-infer::{infer, infer_cached, parse_expr, FunctionTable, ParseCache}` — gone. Inference now lives in the `.hnl` model behind `Kit::typecheck_handl_with_prelude` (ADR-0105), a **different** entry point from `typecheck`'s `check_module`, so the two do not overlap. The old `infer_cached` measured reuse of inference state; the analogue is the `Kit`'s lazily-built Handl engine, so the bench warms the kit and then measures `infer/check_handl`. |
| `core` | ✅ re-targeted | Targeted `handl-typecheck-core::{SchemaInterpreter, Type, is_compatible}` + `schema_json` — gone. Post-ADR-0106 the type universe is the object/sym graph in `handl-value`, so the graph's hot paths stand in for the old operations: `core/decode_value_to_tagged` (wire → graph), `core/encode_tagged_to_value`, `core/check_portable` (the `SchemaInterpreter` successor) and `core/nominal_ancestry` (`has_ancestor`, the `is_compatible` successor). |

## Why a separate crate

`kit-canonical/benches/perf.rs` benchmarks the *assembled kit* (parse+eval of a
long module). This crate benchmarks the *engines* directly, so a regression can
be attributed to the lexer or parser rather than to assembly or eval.
