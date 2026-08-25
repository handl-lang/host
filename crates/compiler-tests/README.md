# handl-compiler-tests

Integration and end-to-end tests for the platform-integrated compiler
pipeline.

- `src/lib.rs` — shared mock `Pass` and `Plugin` helpers.
- `tests/pipeline.rs` — end-to-end tests for `PipelineBuilder` using the
  default `handl-compiler-pass-*` plugins.
