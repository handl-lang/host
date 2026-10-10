# ADR-0085..0094 follow-up: routing the tooling through capability contracts

Status: **done for `fmt` / `lint` / `typecheck` / `test`.** Every step below
except the "third parties" part of step 1's motivation is implemented and
verified:

| Step | State |
| --- | --- |
| 1. Create a capability plugin per tool | `FmtPlugin`, `LintPlugin`, `TypecheckPlugin`, `TestPlugin` all exist |
| 2. Decide the payload schema | Done — all four are `{source, …}`; `lint` carries `language` (ADR-0124) plus `prelude`, `typecheck`/`test` carry `prelude`/`paths` |
| 3. Register them | Done — `host-cli`'s `tooling_host()` registers all four |
| 4. Reroute `host-cli` | Done — `fmt_one`, `lint_one`, `typecheck_one`, and `test_command` all dispatch through the host |
| 5. Plugin + CLI tests | Done — plugin-level tests per tool; CLI smoke-verified per subcommand |

The free functions remain the implementation; each plugin is a thin adapter.

### Deviations from the plan above

- **`TypecheckPlugin` / `TestPlugin` carry no `language` payload.** The plan
  assumed every tool needed the language in the payload. `typecheck` and
  `test` do not: they obtain their language from the single canonical
  assembly path (`Kit::new`), so there is no caller-vs-plugin divergence to
  close. Only `lint` had the hazard (`host-cli` used
  `load_canonical_language()`, the plugin's default was `lint_language()`),
  which is why ADR-0124's payload landed there. See `plugin-ext-typecheck`'s
  crate docs.
- **`fmt` response gained a structured diagnostic.** The reroute returns a
  rendered diagnostic (span included) rather than a flattened error string,
  so `hnl fmt` output is unchanged.
- **`lint` response is `status`-discriminated** (`diagnostics` / `fixed` /
  `error`), matching `fmt`, instead of inferring the case from which field is
  present.
- **`test` splits policy from mechanism.** The CLI keeps default-path
  resolution, flag validation, output stream, and exit code; the plugin does
  discovery + execution + report rendering (the engine's report types are not
  `Serialize`, so the rendered report is returned as text). The ADR-0089
  on-disk cache stays in the plugin, since it belongs to the runner.

## Original planning notes (kept for context)

Both `fmt`/`lint` and `typecheck`/`test` were, before this work, free
functions — `host-cli` called them directly, bypassing the capability layer
even though it already linked the host:

```rust
use handl_platform::{CapabilityRequest, DefaultHost, PlatformError};
// ...but then:
match handl_plugin_ext_fmt::check_source(src) { ... }
handl_plugin_ext_lint::lint_source_with_prelude_and_language(lang, src, prelude)
```

Pre-reroute history, for reference: the tooling surfaces had no `Plugin`
impls (`grep -c 'impl Behavior for\|pub struct .*Plugin'` returned 0 for each)
and `host-cli` called `format_source` / `lint_source_with_prelude` directly.
The dispatch seam already existed — `BasicCli::run` resolves a plugin from
`ctx.plugins` by capability id — so the work was to add the plugins to
resolve *to*, then reroute the subcommands.

Step 5's "identical output before and after" check was done by hand rather
than as a committed test: `hnl fmt`, `hnl lint`, `hnl typecheck`, and
`hnl test` (human + `--json`, `--list`, `--filter`, `--tag`, `--cache`, and a
failing module's exit code) were run against known inputs on both sides of
each reroute.
