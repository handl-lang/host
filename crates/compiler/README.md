# handl-compiler

Compiler pipeline abstractions.

- `pipeline::Pipeline` — runs a sequence of `PassHost`s for the five
  compiler stages (`parse`, `extract`, `resolve`, `typecheck`, `generate`).
- `pipeline::PipelineBuilder` — collects `Pass` objects from a
  `handl-platform::DefaultHost` and assembles a `Pipeline`.

This crate works with `serde_json::Value` for the current phase so that the
pipeline is testable without coupling to real AST/IR types.
