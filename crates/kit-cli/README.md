# handl-kit-cli

A **kit** for building CLI tools with the handl language.

## What is a kit?

A kit is a curated list of plugins. It is not a binary — it is a plugin
that, when loaded by a host binary (like `host-cli`), registers other
plugins into the host's registry. The kit is the single source of truth
for what language features and library modules are available.

## What's in kit-cli?

`kit-cli` composes:

- **`kit-canonical`** — the language itself: lexer, parser, formatter,
  linter, typechecker, evaluator, compiler, and the `ModuleRuntime`
  that accepts library modules.
- **`plugin-lib-os-std`** — OS stdlib: `fs.*`, `env.*`, `path.*`,
  `time.*`, `cmd.*`, `process.*`, `tmp.*`, `socket.*`, `sig.*`.
- **`plugin-lib-utils`** — `math.*`, `str.*`, `bool.*`, `cmp.*`,
  `fn.*`, `type.*`, `option.*`, `result.*`, `encoding.*`, `parse.*`,
  `serialize.*`, `base64.*`, `hex.*`, `json.*`, `toml.*`, `yaml.*`,
  `url.*`.
- **`plugin-lib-collections`** — `array.*`, `set.*`, `map.*`.
- **`plugin-lib-json`**, **`plugin-lib-toml`**, **`plugin-lib-yaml`** —
  standalone serialization modules (also registered by `plugin-lib-utils`,
  but included here for direct use).

## Usage

### From a host binary

```ignore
// host-cli loads kits via config (.handl.toml) or --kit flag.
// To use kit-cli:
//
//   hnl --kit cli eval 'math.abs(-5)'
//   hnl --kit cli run script.hnl
//
// Or in .handl.toml:
//
//   [[kit]]
//   include = "cli"
```

### From Rust

```ignore
use handl_platform::DefaultHost;
use handl_kit_cli::CliKit;

let mut host = DefaultHost::new();
host.register_plugin(CliKit::new());
host.load().unwrap();
```

## Relationship to other kits

| Kit             | Purpose                                          |
| --------------- | ------------------------------------------------ |
| `kit-canonical` | Language operations only (no library modules)    |
| `kit-cli`       | Canonical + OS + utils + collections (CLI tools) |
| `kit-config`    | Canonical + pure modules (sandboxed, no I/O)     |
| `kit-server`    | kit-config + server modules (HTTP, WS, OS, rand) |
| `kit-hello`     | Canonical + hello demo module                    |

`kit-cli` is the starting point for most CLI applications. If you need
server capabilities (HTTP server, WebSocket, etc.), use `kit-server`
instead. If you need a sandboxed pure-functional environment, use
`kit-config`.
