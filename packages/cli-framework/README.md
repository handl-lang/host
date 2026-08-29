# handl CLI Framework

A CLI framework written in [handl](../../crates/kit-canonical/) itself, powered by the [`kit-cli`](../../crates/kit-cli/) kit.

## Overview

This package defines a CLI framework using the handl language. It provides abstractions for defining commands, flags, argument parsing, help generation, and subcommand routing — all as handl expressions.

The `kit-cli` crate provides the runtime modules needed to evaluate these expressions:

- **OS stdlib** — `fs.*`, `env.*`, `path.*`, `cmd.*`, `time.*`, `process.*`, `tmp.*`
- **Utils** — `str.*`, `math.*`, `cmp.*`, `bool.*`, `fn.*`, `type.*`, `option.*`, `result.*`
- **Serialization** — `json.*`, `toml.*`, `yaml.*`
- **Collections** — `array.*`, `set.*`, `map.*`

## Files

| File | Description |
|------|-------------|
| `types.hnl` | Type definitions for `Flag`, `Command`, `ParsedArgs`, `HelpText` |
| `parser.hnl` | Argument parsing — flag extraction, positional args, token classification |
| `help.hnl` | Help text generation — usage strings, flag/subcommand formatting |
| `router.hnl` | Subcommand routing — command resolution and dispatch |
| `cli.hnl` | Main entry point — composes parser, help, and router into `run_cli` |
| `example.hnl` | Example CLI app (`hnl build`, `hnl init`, `hnl test`) |

## Usage

### Running with the handl CLI

```bash
# Use the cli kit
hnl --kit cli run example.hnl 

# Or set it as default
export HANDL_KIT=cli
hnl run example.hnl
```

### Using the kit-cli crate in Rust

```rust
use handl_kit_cli::{CliKit, CliHost, CliLimits};
use handl_platform::DefaultHost;

// As a platform plugin (language operations + library plugins)
let mut host = DefaultHost::new();
host.register_plugin(CliKit::new());
host.load()?;

// As a VM host (expression evaluation with OS + utils modules)
let host = CliHost::new(CliLimits::default());
let ctx = host.new_context();
```

### Defining a CLI in handl

```handl
verbose_flag = {
  name: "verbose",
  short: "v",
  description: "Enable verbose output",
  default: "false",
  required: false
};

build_cmd = {
  name: "build",
  description: "Build the project",
  flags: [verbose_flag],
  subcommands: [],
  handler: "build"
};

root_cmd = {
  name: "myapp",
  description: "My CLI application",
  flags: [verbose_flag],
  subcommands: [build_cmd],
  handler: ""
};
```

## Architecture

```
packages/cli-framework/          crates/kit-cli/
┌──────────────────┐            ┌──────────────────────┐
│  types.hnl       │            │  CliKit (Plugin)     │
│  parser.hnl      │  evaluated │  ├─ CanonicalKit     │
│  help.hnl        │◄──────────►│  ├─ lib.os           │
│  router.hnl      │            │  └─ lib.utils        │
│  cli.hnl         │            │                      │
│  example.hnl     │            │  CliHost (VM Host)   │
└──────────────────┘            │  ├─ OS stdlib modules│
                                │  ├─ Utils modules    │
                                │  └─ Collections      │
                                └──────────────────────┘
```

The handl files define CLI abstractions as expressions. The `kit-cli` crate provides both the platform plugin (language operations) and the VM host (runtime functions) needed to evaluate them.
