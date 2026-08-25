# handl-compiler-pass-resolve

Platform-integrated resolve pass for the `handl-compiler` pipeline.

- `ResolvePass` — a generic `Pass` that marks the JSON payload as resolved.
- `ResolvePlugin` — a `handl-platform::Plugin` wrapper for `ResolvePass`.
- `default_pass_host()` — a `PassHost` pre-registered with `ResolvePass`.
