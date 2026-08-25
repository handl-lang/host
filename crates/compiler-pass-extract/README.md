# handl-compiler-pass-extract

Platform-integrated extract pass for the `handl-compiler` pipeline.

- `ExtractPass` — a generic `Pass` that marks the JSON payload as extracted.
- `ExtractPlugin` — a `handl-platform::Plugin` wrapper for `ExtractPass`.
- `default_pass_host()` — a `PassHost` pre-registered with `ExtractPass`.
