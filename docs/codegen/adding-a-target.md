# Adding a target

A target is registration code plus data — no branch is added to
`plugin-ext-codegen`'s dispatch. The worked example is the **cli**
target (`github.com/handl-lang/tool-codegen-cli`), which ships a
backend and an emitter in one crate.

## Steps

1. **New repo.** Clone the layout of an existing target repo
   (`tool-codegen-*`). Name the crate
   `handl-plugin-codegen-backend-<x>` for a source or
   `handl-plugin-codegen-emitter-<x>` for an output channel — one repo
   may ship both (`tool-codegen-cli` does: source `cli`, emitter
   `hnl-cli`).

2. **Implement the kernel traits.** A source implements
   `Source<I>::parse(&self, req: &EmitContext) -> Result<I,
   CodegenError>` — foreign description in, IR out (usually
   `I = SdkgenModule`). An emitter implements
   `Emitter<I>::materialize(&self, module, req) -> Result<Vec<Change>,
   CodegenError>` plus the `verify` honest-decline gate. Emitters are
   **pure** — they render a desired `Change` set and never write files;
   `Engine::apply` owns all fs mutation. Everything the source/emitter
   needs from argv arrives via `CodegenRequest` at `make` time (see
   below).

3. **Export `register(reg: &mut CodegenRegistry)`.** One function
   declares the source def, the emitter def, and the target record:

   ```rust
   pub fn register(reg: &mut CodegenRegistry) {
       reg.register_source(CodegenSourceDef {
           name: "cli",
           make: make_source,
       });
       reg.register_emitter(CodegenEmitterDef {
           name: "hnl-cli",
           make: |req| Ok(erase_emitter(HnlCliEmitter::new(Arc::new(req.lang.clone())))),
       });
       reg.register_target(CodegenTarget {
           name: "cli",
           source: "cli",
           emitter: "hnl-cli",
           source_filters: false,
           docs: None,
           spec_input: None,
       });
   }
   ```

   - `make: fn(&CodegenRequest) -> Result<Box<dyn AnySource>, String>`
     (same shape for emitters) is where `--config`/`--target`/
     `--include`/`--exclude` become source/emitter state — config files
     load here, not in `parse`, so `docs` passes and spec inputs can
     reload the same config without a constructed source
     (`tool-codegen-c`'s `CBackend::apply_docs_with` is the pattern).
   - Sources/emitters canonicalize output with `req.lang`
     (`Arc<Language>` captured at `make` time); never link
     `handl-kit-data-prelude` from a target repo, and never let
     `Language` cross the kernel boundary inside `EmitContext`.
   - For a self-contained pairing with no record features, prefer
     `reg.register_typed::<S, E, I>(name, source, emitter)` — a source
     `I` that doesn't match its emitter `I` is a compile error.
   - `docs` is an optional post-parse pass
     `fn(&CodegenRequest, &mut SdkgenModule)` — doc-source layering
     belongs there (`tool-codegen-c`'s man pages,
     `tool-codegen-webidl`'s MDN), not in `parse`. It applies to
     `SdkgenModule` IR only.
   - `spec_input` is an optional escape hatch for inputs that are not
     the source's native format (`matches(path)` → `run(req)`); it
     bypasses the `Change` pipeline entirely (direct emit), so
     `plan`/`--check`/`materialize` reject it. The `python` target uses
     it for yaml/json specs — prefer a typed `SpecModule` target once
     the renderer lands.

4. **Wire the host feature.** Add one line to
   `host/crates/plugin-ext-codegen/Cargo.toml`:

   ```toml
   <name> = ["dep:handl-plugin-codegen-<x>", <emitter deps>]
   ```

   plus the `git = "…", branch = "main", optional = true` dependency
   (or a `path` dep while the repo is local-only — the root `[patch]`
   tables resolve sibling checkouts either way) and a `#[cfg(feature)]`
   arm in `linked_targets()` calling `<crate>::register(&mut
   registry)`. Include the feature in `default` to ship it. If the
   target reuses an existing component (`python` composing source `c`),
   only name its id in the target record — resolution is by name, so
   reuse costs no crate dep.

5. **Cross-repo deps follow the float-on-main policy**: git deps on
   `branch = "main"`, resolved from sibling checkouts through the root
   `[patch]` tables during dev, pinned to a rev only at release by
   `scripts/pin-git-revs.py`.

## What a target controls

| Target-record field | Effect |
| --- | --- |
| `name` | `hnl codegen <name>`; also the usage-line entry |
| `source` / `emitter` | Registry ids — may cross repos |
| `source_filters` | Whether `--include`/`--exclude` reach `req`; rejected at exit 2 otherwise |
| `docs` | Post-parse module mutation (doc layering), `SdkgenModule` IR only |
| `spec_input` | Pre-pipeline input path, checked before source resolution on `apply` only |

Errors returned from `make`/`parse`/`materialize`/`verify` are
`Failed` (exit 1); the registry maps unknown names and rejected flags
to `Usage` (exit 2).
