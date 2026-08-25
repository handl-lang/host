# handl-compiler-pass-generate

Platform-integrated generate pass for the `handl-compiler` pipeline.

- `GeneratePass` — a generic `Pass` that wraps the JSON payload in `{"generated": ...}`.
- `GeneratePlugin` — a `handl-platform::Plugin` wrapper for `GeneratePass`.
- `default_pass_host()` — a `PassHost` pre-registered with `GeneratePass`.
