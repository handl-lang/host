# Kits and manifests

A `Kit` is a curated, version-pinned manifest of plugins and default option values. It is the unit of reuse for hosts.

## What a kit is

A kit is not an executable. It is a list of plugin ids, their versions, any default option values, and an optional target. The host loads the kit, applies any overrides from `.handl.toml`, resolves dependencies, and runs `contribute` to a fixed point.

## Example kit manifest

```toml
id = "canonical"
name = "Canonical handl kit"
plugins = [
    "handl-plugin-strings",
    "handl-plugin-numbers",
    "handl-plugin-identifiers",
    "handl-plugin-let",
    "handl-plugin-function",
    "handl-plugin-grammar-infix-pipe",
    "handl-plugin-grammar-apply",
    "handl-plugin-lint-unused-imports",
]
target = "native"

[options]
"comment.style" = "above"
"no-unused-imports.level" = "error"
```

## Why kits matter

Kits make hosts reproducible. A kit pins the exact plugins and defaults it contains. An executable host such as `handl-host-cli` only needs to declare which kits it loads. This separates the curation of language features from the mechanics of the host.

Kits also let users compose languages. A server kit might include HTTP and JSON plugins. A sandbox kit might exclude file-system plugins. A domain-specific kit might add custom grammar and formatter rules. The same `Host` can load any of these without recompilation.

## The kit loading contract

When a host loads a kit:

1. The kit manifest is parsed.
2. Each plugin id is matched against `available_plugins` supplied by the kit crate.
3. The `HostConfig` is built by merging all included kits, applying excludes, and adding direct plugins.
4. `Host::load` resolves dependencies and runs `contribute` to a fixed point.

If a kit requests a plugin that is not in `available_plugins`, loading fails with a clear error. This prevents a kit from silently depending on code the host has not loaded.

## Excluding plugins

A `.handl.toml` can exclude a plugin from a kit without forking the kit:

```toml
[[kit]]
include = "handl-kit-canonical"
exclude = ["handl-plugin-deprecated-grammar"]
```

Exclusion is useful for removing experimental or untrusted plugins. It is not a security boundary; a buggy plugin that remains loaded can still compromise the process.

## Curating a kit

To create a new kit:

1. Create `handl-kit-myname` crate.
2. Write a `KitManifest` that lists the plugins and defaults.
3. Expose the manifest and `available_plugins`.
4. Publish the crate or use it as a path dependency.

A kit should be a closed manifest. The host should not discover plugins on disk at runtime.

## Loading a language from a data directory (per-file kits)

A kit can also be assembled from **authored YAML files** rather than from
plugin ids — the per-file path of ADR 0003:

```rust
use handl_kit_data_canonical::language_from_dir;
let lang = language_from_dir(std::path::Path::new("my-language/"))?;
```

`language_from_dir` reads ten files from the directory and merges them the
same way `language_from_yaml` merges the embedded canonical files:

| file | required |
| --- | --- |
| `tokens.yaml`, `grammar.yaml`, `eval.yaml`, `types.yaml`, `ast.yaml`, `caps.yaml`, `options.yaml`, `lexer_rules.yaml` | yes (empty `[]` is valid) |
| `transforms.yaml`, `lints.yaml`, `lsp.yaml` | optional (absent = none) |

Two things bite when writing these files by hand or from another tool:

- **Enum variants are YAML *tags*, not maps.** `serde_yaml` spells a
  variant as `!Declarative` / `!Create` / `!KeepPath` / `!KindAt` / `!And`,
  where `serde_json` spells it `{"Declarative": …}`. A map in a tagged
  position fails with *"invalid type: map, expected a YAML tag starting
  with '!'"*.
- **Each file is one sequence.** A fragment file is a YAML list of rows
  (`- id: …`), not a bare mapping.

### Worked external example: catopt

The `catopt` project (categorical optimization of computation graphs)
emits its vocabulary in exactly this layout and loads it back:

- generator: `catopt`'s `lab/handl-spike/dump_fragments.py`;
- fixture: `crates/handl-kit-data-canonical/tests/data/catopt-fragments/`
  (eight empty shell files plus `ast.yaml` and `transforms.yaml`);
- test: `crates/handl-kit-data-canonical/tests/catopt_fragments.rs`.

That is the spike that established the boundary: a foreign project's laws
land in `lang.transforms` with their fields intact — **all 83 of them**
(`catopt_full_bank_loads`) — and a foreign law *executes* through the
rewrite engine (`catopt_law_fires_through_the_rewrite_engine`), producing
exactly the term the producer's own engine produces. Rows may carry extra
keys — serde ignores unknown fields — so a producer can ride its own
authoritative record alongside the handl-shaped view (catopt uses a
`catopt:` key for this).

**Not yet reachable from the CLI.** `hnl` selects kits through
*registered emitters*; a data directory is reachable through this API (or
a host that calls it). `FragmentSource::DataFile` is modeled in
`handl-kit-data` but returns `LangError::NotImplemented` — the runtime
half of the seam is still to come.
