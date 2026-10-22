# Adding a target

A target is registration code plus data — no branch is added to
`plugin-ext-sdkgen`'s dispatch. The worked example is **sdkgen-cli**
(`github.com/handl-lang/sdkgen-cli`), the newest target.

## Steps

1. **New repo.** Clone the layout of an existing target repo
   (`sdkgen-*`). Name the crate `handl-plugin-sdkgen-backend-<x>` for a
   parser or `handl-plugin-sdkgen-emitter-<x>` for an output channel —
   one crate may ship both (sdkgen-cli does: backend `cli`, emitter
   `hnl-cli`).

2. **Implement the traits.** A backend implements
   `SdkgenBackend::parse_header(&self, path) -> Result<SdkgenModule,
   SdkgenError>` — foreign description in, IR out. An emitter implements
   `Emitter::emit(&self, module, output)`. Everything the emitter needs
   from argv arrives via `SdkgenRequest` at construction time (see
   `make` below).

3. **Export `register(reg: &mut SdkgenRegistry)`.** One function
   declares the backend def, the emitter def, and the target record:

   ```rust
   pub fn register(reg: &mut SdkgenRegistry) {
       reg.register_backend(SdkgenBackendDef {
           name: "cli",
           make: make_backend,
       });
       reg.register_emitter(SdkgenEmitterDef {
           name: "hnl-cli",
           make: |req| Ok(Box::new(HnlCliEmitter::new(Arc::new(req.lang.clone())))),
       });
       reg.register_target(SdkgenTarget {
           name: "cli",
           backend: "cli",
           emitter: "hnl-cli",
           source_filters: false,
           docs: None,
           spec_input: None,
       });
   }
   ```

   - `make: fn(&SdkgenRequest) -> Result<Box<dyn SdkgenBackend>,
     String>` is where `--config`/`--target`/`--include`/`--exclude`
     become backend state — config files load here, not in
     `parse_header`, so `docs` passes and spec inputs can reload the
     same config without a constructed backend (sdkgen-c's
     `CBackend::apply_docs_with` is the pattern).
   - Emitters canonicalize output with `req.lang` (`FileEmitter::new`,
     `HnlCliEmitter::new` take `Arc<Language>`); never link
     `handl-kit-data-prelude` from a target repo.
   - `docs` is an optional post-parse pass
     `fn(&SdkgenRequest, &mut SdkgenModule)` — doc-source layering
     belongs there (sdkgen-c's man pages, sdkgen-webidl's MDN), not in
     `parse_header`.
   - `spec_input` is an optional escape hatch for inputs that are not
     the backend's native format (`matches(path)` → `run(req)`); the
     `python` target uses it for yaml/json specs.

4. **Wire the host feature.** Add one line to
   `host/crates/plugin-ext-sdkgen/Cargo.toml`:

   ```toml
   <name> = ["dep:handl-plugin-sdkgen-<x>", <emitter deps>]
   ```

   plus the `git = "…", branch = "main", optional = true` dependency and
   a `#[cfg(feature)]` arm in `linked_targets()`. Include the feature in
   `default` to ship it. If the target reuses an existing component
   (`python` composing backend `c`), only name its id in the target
   record — resolution is by name, so reuse costs no crate dep.

5. **Cross-repo deps follow the float-on-main policy**: git deps on
   `branch = "main"`, resolved from sibling checkouts through the root
   `[patch]` tables during dev, pinned to a rev only at release by
   `scripts/pin-git-revs.py`.

## What a target controls

| Target-record field | Effect |
| --- | --- |
| `name` | `hnl sdkgen <name>`; also the usage-line entry |
| `backend` / `emitter` | Registry ids — may cross repos |
| `source_filters` | Whether `--include`/`--exclude` reach `req`; rejected at exit 2 otherwise |
| `docs` | Post-parse module mutation (doc layering) |
| `spec_input` | Pre-pipeline input path, checked before backend resolution |

Errors returned from `make`/`run`/`emit` are `Failed` (exit 1); the
registry maps unknown names and rejected flags to `Usage` (exit 2).
