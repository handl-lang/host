# Security and trust

The handl platform is designed for trusted, curated plugin graphs. It does not provide sandboxing or permission manifests by default. This page explains the threat model and the practical limits of the design.

## Threat model

- All plugins run in the same process.
- A plugin is a Rust crate or a dynamically loaded module.
- A malicious or buggy plugin can compromise the entire process if it runs native code.
- The platform does not provide process-level sandboxing by default.

If you load a plugin, you are trusting it with the same privileges as the host process.

## What the design gives us

The platform does provide some safety properties:

- **Type-safe rule boundaries.** `contribute::<P>(rule)` is checked at compile time. A `Format` rule cannot be contributed to a `Linter`.
- **Explicit dependency declaration.** `requires` and `optional` make the plugin graph visible and auditable.
- **Closed kit manifests.** A `Kit` lists every plugin it contains. The host is not surprised by hidden plugins.
- **Option validation.** `OptionSchema` declares `possible_values`. The `Options` plugin rejects values outside the declared set.
- **Plugin exclusion.** A `.handl.toml` can `exclude` specific plugins from a kit, so users can remove untrusted or experimental code.

## What the design does not give us

The design is not a security boundary for untrusted code:

- **No process isolation.** A plugin that uses `unsafe` or calls arbitrary syscalls can crash or compromise the `Host`. Rust's type system does not prevent this.
- **No automatic capability restrictions.** By default, a `Plugin` can `contribute` to any `RuleAcceptor` and a `Library`/`Target` plugin can `register_plugin`.
- **No plugin signing.** Kits and libraries are trusted by inclusion, not by cryptographic signature.

## Recommended security measures

If you curate or deploy hosts, consider the following:

1. **Capability-limited `ContributionContext`.** The `Host` can hand different `ContributionContext` flavours to different plugins:
   - `Library` and `Target` plugins get `register_plugin`.
   - Rule plugins get `contribute` to public `RuleAcceptor`s only.
   - Untrusted plugins get a read-only or `contribute`-only context.

2. **Trusted kit curation.** `handl-kit-*` crates should be reviewed packages. A user `.handl.toml` should only pull from known kit crates or explicitly list direct plugins.

3. **No `unsafe` in canonical plugins.** The `handl-platform` and `handl-plugin-ext-*` crates should be `#![forbid(unsafe_code)]` where possible.

4. **Reproducible, version-pinned kits.** `Kit` manifests pin plugin versions. A host can refuse to load a kit whose dependency tree does not match the pinned hashes.

## Out of scope

Sandboxed targets, signed plugins, and permission manifests are not required for the initial design. They are tracked in `project/BACKLOG.md`.

The design is **safe for trusted, curated plugin graphs** and **not safe for arbitrary untrusted native plugins**. If you need untrusted third-party plugins, use a sandboxed `Target` or an external sandbox.
