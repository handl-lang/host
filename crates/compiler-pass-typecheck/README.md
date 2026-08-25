# handl-compiler-pass-typecheck

Platform-integrated typecheck pass for the `handl-compiler` pipeline.

- `TypecheckPass` — a generic `Pass` that marks the JSON payload as typechecked.
- `TypecheckPlugin` — a `handl-platform::Plugin` wrapper for `TypecheckPass`.
- `default_pass_host()` — a `PassHost` pre-registered with `TypecheckPass`.
