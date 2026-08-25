# handl-compiler-common

Common compiler abstractions used by the `handl-compiler` and `handl-compiler-pass-*` crates.

- `Pass` — a `Send + Sync` trait for JSON-to-JSON compiler passes.
- `PassHost` — runs a sequence of `Pass`es in order.
- `CompilerError` — shared compiler error type.

This crate intentionally works with `serde_json::Value` so that pass hosts and
pipeline builders can be tested without depending on real AST/IR types.
