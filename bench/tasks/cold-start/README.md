# cold-start

Print `"Hello, world!"` and exit. Measures pure process startup + runtime
initialization overhead — no meaningful computation.

## Expected output

```
Hello, world!
```

## Why this task

- Isolates startup cost from compute cost.
- handl pays for Rust process startup **plus** kit assembly (lexing/parsing
  the canonical language data, building the runtime). This is the real-world
  "how long does `hnl run file.hnl` take for a tiny file?" number.
- bun and node pay V8/JavaScriptCore startup. Python pays CPython startup.
  Rust and C pay only the OS process + `printf`.
- No heavy-work mode — cold-start IS the startup measurement.
