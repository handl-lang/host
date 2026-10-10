# ADR-0124: Handing a `Language` to capability plugins

## Context

The `plugin-ext-*` tooling plugins (`fmt`, `lint`, and the proposed
`typecheck`/`test`) split into two groups:

- **language-free** — `format_source(src)`, `lint_source(src)`. They build the
  canonical language internally. These already work through the capability
  seam (`FmtPlugin`, `LintPlugin`).
- **language-bound** — `format_source_with_language(lang, src)`,
  `lint_source_with_prelude_and_language(lang, src)`,
  `check_module(lang, src)`, and the test runner. They take the `Language`
  from their **caller**.

The second group cannot be expressed today. A `Language` is a large,
non-`serde` Rust value (tokens, grammar, eval schemes, AST shapes,
namespaces), while a capability payload is `serde_json::Value`. So
`hnl fmt --language <name>`, `hnl typecheck`, and `hnl test` — the cases that
matter most — have no path through the contract, and `host-lsp` (which works
with workspace language variants, ADR-0003) cannot reuse the tooling at all.

This is the blocker recorded in
[`project/tasks/0085-tooling-capability-wiring.md`](../tasks/0085-tooling-capability-wiring.md):
"the payload cannot carry a Language".

A complication fixes the shape of the answer: **`platform` deliberately does
not depend on `handl-language`.** `platform` defines `Plugin`, `Capability`,
`Host`, and `Kit`, and its `Kit` is manifest-level (`KitManifest` +
`Vec<PluginManifest>`) with no language payload. So the seam cannot simply
grow a field of type `Language` without making the generic plugin host
depend on the language layer.

## Decision

Add a **non-serialized, type-keyed side-channel** to the capability request,
plus a **resolver trait** that lives in the language layer. Nothing
language-specific moves into `platform`.

### 1. A typed, in-process side-channel (platform, language-agnostic)

```rust
pub struct CapabilityRequest {
    pub capability: CapabilityId,
    pub payload: serde_json::Value,
    /// In-process host objects. Never serialized: these are `Arc`s to live
    /// runtime values (a `Language`, a resolver, …) that cannot cross a
    /// process boundary. Empty for out-of-process callers.
    #[serde(skip)]
    host_objects: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl CapabilityRequest {
    pub fn insert_host_object<T: Any + Send + Sync>(&mut self, v: Arc<T>);
    pub fn host_object<T: Any + Send + Sync>(&self) -> Option<Arc<T>>;
}
```

`platform` depends only on `Any`, so it stays language-agnostic.

### 2. A resolver trait in the language layer

```rust
/// Resolves a kit name (or the host default when `None`) to a language.
pub trait LanguageResolver: Send + Sync {
    fn language_for(&self, kit: Option<&str>) -> Result<Arc<Language>, String>;
}
```

The host inserts its implementation (the same data that already backs
`hnl --language <name>`), so a **custom kit** resolves, not just canonical.

### 3. A single accessor the plugins use

```rust
fn request_language(req: &CapabilityRequest, ctx: &PluginContext)
    -> Result<Arc<Language>, PlatformError>;
```

Resolution order:

1. an `Arc<Language>` placed directly on the request (fast path, exact);
2. the request's `LanguageResolver`, asked for `payload["kit"]` (or the
   default when absent/`null`);
3. otherwise `PlatformError::InvalidMetadata` naming *both* ways to supply a
   language, so a plugin author who forgets gets an actionable error rather
   than a silent canonical fallback.

A new optional payload field makes the request self-describing for
out-of-process callers:

```json
{ "source": "…", "kit": "config" }
```


## Consequences

**Positive.**

- `host-cli` and `host-lsp` pass the real language with **zero serialization
  and full fidelity**, including a user kit loaded from `--language <name>`.
- Out-of-process / WASM plugins still work for anything they can build
  themselves (canonical today), by name.
- `platform` keeps its language-agnosticism; no new dependency edge.
- One mechanism, not a per-tool workaround.

**Negative / constraints.**

- The side-channel is **in-process only**. An out-of-process plugin asking
  for a language it cannot build must fail; that limitation has to be
  documented rather than papered over.
- **Two ways to supply a language** (direct `Arc` or resolver) is a small
  footgun. Mitigation: the accessor above is the *only* way plugins read it,
  and the missing-case error names both.
- `TypeId`-keyed maps are untyped by construction: a wrong-type `insert` is
  silently ignored (the lookup returns `None`). Mitigation: only the helper
  inserts; it is not part of the public plugin API.

## Alternatives rejected

- **`platform` depends on `handl-language`; `CapabilityRequest.language:
  Option<Arc<Language>>`.** Simplest to write, but makes the generic plugin
  host depend on the language layer — every host, including ones that never
  touch handl syntax, would link it. Rejected on layering.
- **Payload carries the YAML fragments; the plugin rebuilds the language.**
  Serializable, but loses anything not expressible as fragments, and kit
  assembly is far too slow to repeat per call.
- **The plugin builds the canonical language itself.** Works for `fmt`/`lint`
  defaults, fails the custom-kit and `host-lsp` cases that motivate the work,
  and would force a *linter* to depend on `kit-canonical`.
- **Keep language-bound operations out of the capability layer.** Leaves
  `host-lsp` unable to reuse the tooling, i.e. fails the stated goal.

## Migration

1. Land the platform side-channel + `LanguageResolver` (no behaviour change;
   nothing reads it yet).
2. Add `typecheck` and `test` plugins using the accessor; extend `fmt`/`lint`
   to accept the optional language.
3. Reroute `host-cli`'s `--language` variants and `hnl typecheck`/`hnl test`
   through the seam.
4. **Test:** a *custom* kit driven through the capability path must produce
   identical results to the direct call. Without that, "the seam is
   equivalent" is untested — the same gap ADR-0123's acceptance test closed.

## Status

**Proposed.** Not implemented. One open sub-question for review: the channel
lives on `CapabilityRequest` per call (precise — a request names one kit) or
on `PluginContext` per host run (simpler, one resolver for all requests).
Recommendation is per-request, since a single host can serve several kits.
