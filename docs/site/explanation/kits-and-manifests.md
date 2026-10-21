# Kits and manifests

A `Kit` is a curated, version-pinned manifest of plugins and default
option values. It is the unit of reuse for hosts.

## What a kit is

A kit is not an executable. It is a list of plugin ids, their versions,
any default option values, and an optional target. The host loads the
kit, applies any overrides from `.handl.toml`, resolves dependencies,
and runs `contribute` to a fixed point.

## Example kit manifest

```toml
id = "my-kit"
name = "My kit"
plugins = [
    "my-lang-tokens",
    "my-lang-grammar",
    "my-lang-eval",
    "my-lint-rules",
]
target = "tree-walk"

[options]
"comment.style" = "above"
"no-unused-imports.level" = "error"
```

## Why kits matter

Kits make hosts reproducible. A kit pins the exact plugins and defaults
it contains, so an executable host only needs to declare which kits it
loads. This separates the curation of language features from the
mechanics of the host.

Kits also let users compose languages. A server kit might include HTTP
and JSON plugins; a sandbox kit might exclude file-system plugins; a
domain-specific kit might add custom grammar and formatter rules. The
same `Host` can load any of these without recompilation.

## The kit loading contract

When a host loads a kit:

1. The kit manifest is parsed.
2. Each plugin id is matched against `available_plugins` supplied by the
   kit crate.
3. The `HostConfig` is built by merging all included kits, applying
   excludes, and adding direct plugins.
4. `Host::load` resolves dependencies and runs `contribute` to a fixed
   point.

If a kit requests a plugin that is not in `available_plugins`, loading
fails with a clear error — a kit cannot silently depend on code the host
has not loaded.

## Excluding plugins

A `.handl.toml` can exclude a plugin from a kit without forking the kit:

```toml
[[kit]]
include = "my-kit"
exclude = ["my-experimental-plugin"]
```

Exclusion is useful for removing experimental or untrusted plugins. It
is not a security boundary; a buggy plugin that remains loaded can still
compromise the process.

## Curating a kit

To create a new kit:

1. Write a `KitManifest` that lists the plugins and defaults.
2. Expose the manifest and `available_plugins`.
3. Publish the crate or use it as a path dependency.

A kit should be a closed manifest — the host does not discover plugins
on disk at runtime.

## A language kit as data files

A kit can also be assembled from **authored YAML files** rather than
from plugin ids — the per-file path of the language-as-data model
(see [Language as data](./language-as-data)). The published
[`handl-kit-prelude-lang`](https://github.com/handl-lang/kit-prelude-lang)
crate exposes this directly:

```rust
use handl_kit_prelude_lang::language_from_dir;
let lang = language_from_dir(std::path::Path::new("my-language/"))?;
```

`language_from_dir` reads the fragment files from the directory and
merges them the same way `language_from_yaml` merges the embedded
canonical fragments:

| file | required |
| --- | --- |
| `tokens.yaml`, `grammar.yaml`, `eval.yaml`, `types.yaml`, `ast.yaml`, `caps.yaml`, `options.yaml`, `lexer_rules.yaml` | yes (empty `[]` is valid) |
| `transforms.yaml`, `lints.yaml`, `lsp.yaml` | optional (absent = none) |

Two things bite when writing these files by hand or from another tool:

- **Enum variants are YAML *tags*, not maps.** `serde_yaml` spells a
  variant as `!Declarative` / `!Create` / `!KeepPath` / `!KindAt` /
  `!And`, where `serde_json` spells it `{"Declarative": …}`. A map in a
  tagged position fails with *"invalid type: map, expected a YAML tag
  starting with '!'"*.
- **Each file is one sequence.** A fragment file is a YAML list of rows
  (`- id: …`), not a bare mapping.

### Foreign vocabularies

Because fragments are data with a stable schema, an external project can
emit its own vocabulary in this layout and have it execute through the
same engines — the research stack
([`graphopt`](https://github.com/handl-lang/graphopt)) does
exactly this, dumping transform and AST fragments that merge into a
`Language`. Rows may carry extra keys — serde ignores unknown fields —
so a producer can ride its own authoritative record alongside the
handl-shaped view.

**Not every path is wired yet.** Loading a data directory through a
high-level kit selector is modeled but not implemented end-to-end; the
API above is the reachable seam today.
