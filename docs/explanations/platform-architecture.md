---
title: Platform architecture
---

# Platform architecture

Handl is now built around a small, stable platform layer. The rest of the workspace—compiler, VM, source handling, and CLI—consumes that layer instead of hard-coding its own contracts.

## The layers

- **Platform** — the mechanisms and contracts that every other crate shares. It lives in `crates/platform` (`handl-platform`) and defines `Capability`, `Plugin`, `Extension`, `Kit`, `Host`, and `Boundary`.
- **Capability** — something the platform can do. Each `handl-capability-*` crate is a normal plugin that implements one capability (`fmt`, `lint`, `typecheck`, `eval`, `compile`, `build`, `plugin`, `kit`, `extension`).
- **Plugin** — the unit that implements a capability. It registers itself with the platform and provides `CapabilityMetadata` describing what it does.
- **Kit** — a bundle of plugins. Kits are defined in the platform but fully wired later.
- **Framework** — an opinionated composition of kits and hosts. This is intentionally deferred for now.
- **Host** — anything that consumes capabilities. `handl-cli` is the default command-line host; LSP servers, IDEs, web UIs, and CI systems can be hosts too.
- **Boundary** — a packaging and communication concept. A boundary can be a module, package, process, worker, server, client, RPC link, or deployment.

## Capabilities are metadata-driven

A plugin returns arbitrary, JSON-serializable `CapabilityMetadata`. The platform itself does not define what keys are in that metadata; each host decides how to interpret it. The CLI host reads a `command` field and uses it to generate a `clap` subcommand, but an LSP host could read `lsp-method` instead.

A typical `CapabilityMetadata` value for a CLI command looks like this:

```json
{
  "command": "compile",
  "description": "Compile a Handl module",
  "aliases": ["build"]
}
```

Because the CLI does not hard-code `compile` or `build`, any plugin that returns this metadata automatically gets a `handl compile` command. Aliases appear as extra subcommand names.

## How a new plugin becomes a command

A new `Plugin` implementation registers a capability and returns metadata:

```rust
use handl_platform::{Capability, CapabilityId, CapabilityMetadata, Plugin, PluginContext};

pub struct FormatPlugin;

impl Plugin for FormatPlugin {
    fn id(&self) -> CapabilityId {
        CapabilityId::from("fmt")
    }

    fn metadata(&self) -> CapabilityMetadata {
        serde_json::json!({
            "command": "fmt",
            "description": "Format a Handl document",
            "aliases": ["format"]
        })
    }

    fn invoke(&self, ctx: &mut PluginContext, request: serde_json::Value)
        -> Result<serde_json::Value, handl_platform::Error>
    {
        // Capability logic here
        Ok(request)
    }
}
```

When the `handl-cli` host starts, it loads the default `Host`, walks the `PluginRegistry`, and reads each plugin's `CapabilityMetadata`. It sees `command: "fmt"` and creates:

```sh
handl fmt
handl format    # alias
```

No changes to `handl-cli` are required.

## The CLI is just one host

The `handl` command-line tool is deliberately thin. It knows how to:

1. Build the default `Host` and its registries.
2. Read `CapabilityMetadata` from every registered plugin.
3. Turn metadata into `clap` subcommands, options, and aliases.

Everything else—parsing, formatting, linting, evaluation, typechecking, compiling—lives in capability plugins. Management of plugins, kits, and extensions is also implemented as capabilities, so the CLI has no special administrative commands.

## Why this matters

- The platform layer is the only shared contract, so dependency direction is always clear.
- New capabilities appear automatically in every host that reads `CapabilityMetadata`.
- Hosts can be tiny: the CLI is a few hundred lines because the real work is in plugins.
- Capabilities can move to separate repositories later without changing the platform contracts.
