# Set up the platform

This guide gets the published platform repos building on your machine.

## Requirements

- A recent stable Rust toolchain (1.80 or newer) and `cargo`.
- `git`.

## Clone and build

Clone the platform workspace and the kernel as siblings — `host`'s
`[patch]` table resolves `kernel` crates from `../kernel`:

```sh
git clone https://github.com/handl-lang/host
git clone https://github.com/handl-lang/kernel

cd host
cargo check --workspace
cargo test --workspace
```

Clone the engines and language data only as you need them:

```sh
git clone https://github.com/handl-lang/engine-exec
git clone https://github.com/handl-lang/engine-exec-tree-walk
git clone https://github.com/handl-lang/engine-lex
git clone https://github.com/handl-lang/engine-parse
git clone https://github.com/handl-lang/engine-transform
git clone https://github.com/handl-lang/kit-prelude-lang
```

## For a downstream crate

Depend on the platform directly — no checkout needed:

```toml
[dependencies]
handl-platform = { git = "https://github.com/handl-lang/host" }
serde_json = "1"
```

## What is not here

The `hnl` CLI, editor extensions, the curated kits, and most of the
standard library are not yet released. The toolchain *engines* are
public, though — `engine-fmt`, `engine-lint`, and `engine-typecheck` are
git-dependency crates you can add to a Rust project, and a handful of
`lib-*` packages (`lib-char`, `lib-char-native`, `lib-desugar`,
`lib-loader`, `lib-str-handl`) are public language packages.

## Next steps

- [Build a host](./build-a-host)
- [Write a plugin](./write-a-plugin)
