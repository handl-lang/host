# handl `lib-utils` curated examples

The full per-module `lib-utils` examples now live colocated with each crate,
under `crates/lib-utils/examples/<module>/`.

This directory keeps a few high-level/intro examples that do not map to a single crate.

- `hello.hnl` — a minimal handl expression to get started.

## Running with `host-cli`

```sh
cargo build -p handl-host-cli    # binary: target/debug/hnl
hnl eval hello.greet("world")    # → Hello, world!
hnl eval json '"hello world"'    # → "hello world"
```

`hnl eval` only handles single expressions, not full `.hnl` files.
Running `hello.hnl` (which has declarations and references) requires a
script execution capability not yet implemented.
