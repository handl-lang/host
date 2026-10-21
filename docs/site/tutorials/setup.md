# Set up the platform

This tutorial gets the published platform building on your machine. By
the end you will have the `host` workspace checked out and verified.

## Prerequisites

- A recent stable Rust toolchain (1.80 or newer) and `cargo`.
- `git`.

## Clone the repos

The platform is split across sibling repositories. Start with `host` —
the platform workspace — and `kernel`, which holds the calculus and the
`Language` data schema:

```sh
git clone https://github.com/handl-lang/host
git clone https://github.com/handl-lang/kernel
```

Clone them as siblings — `host`'s `[patch]` table resolves `kernel`
crates from `../kernel` for local development:

```text
work/
├── host/
└── kernel/
```

The generic engines live in their own repos; clone the ones you need:

```sh
git clone https://github.com/handl-lang/engine-lex
git clone https://github.com/handl-lang/engine-parse
git clone https://github.com/handl-lang/engine-transform
git clone https://github.com/handl-lang/engine-exec
git clone https://github.com/handl-lang/engine-exec-tree-walk
```

And the canonical language data:

```sh
git clone https://github.com/handl-lang/kit-prelude-lang
```

## Build and test

```sh
cd host
cargo check --workspace
cargo test --workspace
```

The `handl-platform` contract crate is `crates/platform`; everything in
the tutorials builds on it.

## What is not here

The `hnl` CLI, the editor extensions, the toolchain engines
(formatter, linter, typechecker, compilers), the curated kits, and the
standard library are not yet released — there is nothing to install.
What you can build today is a host binary of your own, which is the
next tutorial.

## Next steps

- [Build a host](./build-a-host) — create a binary that composes plugins
  and runs a capability.
- [Write a plugin](./write-a-plugin) — implement `Plugin` and contribute
  a rule.
