---
title: Handl Language
---

# Handl Language

**The categorical language platform** — a closed arrow-calculus kernel,
a data-driven language engine, and a plugin-composed host.

The published pieces today are the machinery. The handl surface language
— syntax, the standard library, and the `hnl` toolchain — is in
development and deliberately not documented here.

## From foundation to surface

| layer | what it is | where |
| --- | --- | --- |
| **Kernel** | The closed arrow calculus — `Term`/`Value`, `Plugin`/`Host`/`Operation`/`Behavior` primitives, the `Language` data schema | [`kernel`](https://github.com/handl-lang/kernel) |
| **Engines** | Generic, data-driven machinery for lex, parse, transform, and exec — dispatch on `Language` data, never on hardcoded constructs | [`engine-*`](https://github.com/handl-lang) |
| **Platform** | `handl-platform` contracts — hosts, kits, options, capability dispatch, and the `plugin-ext-*` operation surfaces | [`host`](https://github.com/handl-lang/host) |
| **Language data** | The canonical eval-path `Language` as YAML fragments | [`kit-prelude-lang`](https://github.com/handl-lang/kit-prelude-lang) |

## Documentation

- [Tutorials](./tutorials/) — set up the platform, build a host, write a
  plugin.
- [How-to guides](./how-to/) — focused recipes for the platform.
- [Explanations](./explanation/) — the kernel, the engine model, and the
  composable host, plus the papers.
- [Reference](./reference/) — host concepts, the kernel substrate, and
  the tensor host.
