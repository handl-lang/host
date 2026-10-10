# ADR-0085..0094 follow-up: routing the tooling through capability contracts

Status: **scoped, not started.** This is TODO.md's "High priority — bootstrap
tooling integration" item:

> Wire `plugin-ext-fmt` / `plugin-ext-lint` / `plugin-engine-typecheck` /
> `plugin-engine-test` through the existing `plugin-ext-*` capability
> contracts so `host-lsp`, `host-cli`, and third parties can reuse them

## Finding: the plugins do not exist yet

The item reads like a wiring task. It is not. Investigation shows:

**1. The tooling surfaces are free functions, not plugins.** None of
`plugin-ext-fmt`, `plugin-ext-lint`, `plugin-ext-eval`, `plugin-ext-build`,
`plugin-ext-compile` declares a `Plugin` or `Behavior` impl:

```console
$ for c in plugin-ext-fmt plugin-ext-lint plugin-ext-eval plugin-ext-build plugin-ext-compile; do
    grep -c 'impl Behavior for\|pub struct .*Plugin' crates/$c/src/lib.rs; done
0
0
0
0
0
```

They export `format_source` / `check_source` / `lint_source_with_prelude` /
`fix_source_with_prelude` and so on.

**2. `host-cli` therefore calls them directly**, bypassing the capability
layer even though it already links the host:

```rust
use handl_platform::{CapabilityRequest, DefaultHost, PlatformError};
use handl_plugin_ext_cli::BasicCli;
// ...but then:
match handl_plugin_ext_fmt::check_source(src) { ... }
handl_plugin_ext_lint::lint_source_with_prelude_and_language(lang, src, prelude)
```

**3. The dispatch seam that *would* carry them already exists.**
`BasicCli::run` takes a `CapabilityRequest` with a `{command, args}` payload,
resolves the target plugin from `ctx.plugins` by capability id, and delegates:

```rust
let command_id = CapabilityId::from(command);
let plugin = ctx.plugins.resolve(&command_id)?;
plugin.run(&CapabilityRequest { capability: command_id, payload }, ctx)
```

So the intended shape is `{command: "fmt", args: [...]}` → resolve `fmt` →
run. What is missing is the plugins to resolve *to*.

## Work required

1. **Create a capability plugin per tool** — `FmtPlugin`, `LintPlugin`,
   `TypecheckPlugin`, `TestPlugin` — each `impl Behavior` with
   `capabilities()`, `metadata()`, and a `run()` that unpacks
   `{args, ...}` from the payload and calls the existing free function.
   The free functions stay as the implementation; the plugin is a thin
   adapter, so this is additive and low-risk per plugin.
2. **Decide the payload schema** for each — `fmt` needs `{path?, source,
   check, write}`, `lint` needs `{source, fix, prelude, language?}`,
   `typecheck` needs `{source, kit}`. Note `plugin-ext-lint` already has a
   `*_with_language` variant, i.e. the language/kit is an input; the payload
   must carry it (or the plugin must resolve it).
3. **Register them** in the host/registry that `host-cli` builds, so
   `ctx.plugins.resolve(&CapabilityId::from("fmt"))` succeeds.
4. **Refactor `host-cli`'s subcommand bodies** to build a
   `CapabilityRequest` and dispatch through the host instead of calling the
   free functions. Keep the free functions as the fallback seam.
5. **Tests** — a plugin-level test per tool, plus a CLI-level test proving
   `hnl fmt` and `hnl lint` produce identical output on a known input before
   and after the rerouting.

## Why this is not done here

Steps 4 and 5 touch `host-cli/src/main.rs` (2000+ lines) and the primary
user-facing binary. Doing them half-way risks breaking `hnl` for every user,
and the environment available for this session could not support iterative
verification (see below). Steps 1–3 are the safe, additive subset and are
the natural first PR; step 4 should follow once the plugins exist and are
individually tested.

## Environment note (why verification was constrained)

The workspace debug target is ~8.5 GB and the volume was at 100% (2 MB free)
for most of the session; the full `cargo test --workspace` build cannot fit
and had to be abandoned in favour of per-crate runs. Builds also serialize on
a global `flock` held by the `cargo` wrapper in `~/.local/bin`, so unrelated
projects contend. Verification should be redone on an uncontended box before
merging step 4.
