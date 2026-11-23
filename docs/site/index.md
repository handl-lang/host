---
title: Handl Language
---

# Handl Language

**The categorical language platform** — a closed arrow-calculus kernel,
a data-driven language engine, and a plugin-composed host.

The published pieces today are the machinery — the `kernel` schema and
values, the whole `engine`/`tool-*`/`exec-*` operation layer (lex, parse,
transform, exec — tree-walk, bytecode, and Cranelift — plus the
`fmt`/`lint`/`typecheck` toolchain engines), the `host` platform
workspace, the canonical `kit-prelude-lang` language data, and
`host-tensor`. The `hnl` CLI, the `ext-*` extensions, the curated
`kit-*` kits, and most of the standard library are still in
development.

## From foundation to surface

| layer | what it is | where |
| --- | --- | --- |
| **Kernel** | The `Term` arrow calculus (`engine`/`hnl-engine`), the `Language` data schema, and the tagged `Value` model | [`kernel`](https://github.com/handl-lang/kernel), [`engine`](https://github.com/handl-lang/engine) |
| **Engines** | Generic, data-driven machinery for lex, parse, transform, typecheck, exec, fmt, and lint — dispatch on `Language` data, never on hardcoded constructs; interchangeable eval backends (tree-walk, bytecode, Cranelift) | [`tool-*`/`exec-*`/`engine-*`](https://github.com/handl-lang) |
| **Platform** | `handl-platform` contracts — `Plugin`/`DefaultHost`/`Operation`/`Behavior`/`RuleAcceptor`/`Contribution` primitives, hosts, kits, options, capability dispatch, and the `plugin-ext-*` operation surfaces | [`host`](https://github.com/handl-lang/host) |
| **Language data** | The canonical eval-path `Language` as YAML fragments | [`kit-prelude-lang`](https://github.com/handl-lang/kit-prelude-lang) |

## Documentation

- [Tutorials](./tutorials/) — set up the platform, build a host, write a
  plugin.
- [How-to guides](./how-to/) — focused recipes for the platform.
- [Explanations](./explanation/) — the kernel, the engine model, and the
  composable host, plus the papers.
- [Reference](./reference/) — host concepts, the kernel substrate, and
  the tensor host.
