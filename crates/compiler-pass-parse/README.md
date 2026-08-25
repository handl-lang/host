# handl-compiler-pass-parse

Platform-integrated parse pass for the `handl-compiler` pipeline.

- `ParsePass` — a generic `Pass` that marks the JSON payload as parsed.
- `ParsePlugin` — a `handl-platform::Plugin` wrapper for `ParsePass`.
- `default_pass_host()` — a `PassHost` pre-registered with `ParsePass`.
