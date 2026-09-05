# ADR-0089: Bootstrap Test Runner (`plugin-engine-test`)

## Status

**Rewritten.** Part of the bootstrap tooling strategy
([ADR-0085](./0085-bootstrap-tooling-strategy.md)). Depends on
[ADR-0074 — Arrow Surface Syntax v0.1](./0074-arrow-syntax-v0_1-surface-redesign.md),
[ADR-0044 — Language as Data](./0044-language-as-data-and-runtime-split.md)
(test-language fragment), [ADR-0081 — Data-driven AST child roles](./0081-data-driven-ast-child-roles.md),
and the eval engine (`plugin-engine-eval`). The test **framework library**
(`assert`/`property`/`benchmark`/`fuzz`/`snapshot`) moves to handl under
[ADR-0092](./0092-test-framework-in-handl.md); the future handl port of
this runner is [RFC-0005](../rfcs/0005-test-runner-in-handl.md).

> **Scope.** This ADR governs the **test runner** — the Rust machinery that
> discovers, plans, schedules, executes, and reports tests. It does **not**
> govern the test **framework library** (the `assert`/`property`/… primitives
> that handl test code calls); that is
> [ADR-0092](./0092-test-framework-in-handl.md), which migrates the framework
> to handl and deletes `plugin-lib-test`.

## Context

The original ADR-0089 specified a minimal, single-threaded runner that
discovered tests by the **`test_*` naming convention** (workaround for v0.1
grammar having no `test` keyword), executed in source order with the
tree-walk eval engine, and reported PASS/FAIL/SKIP + JSON. It deferred
parallelism, properties, benchmarks, filtering, caching, and provider
extensibility to "RFC-0005, the future handl port."

Two problems with that shape:

1. **Building a toy runner and bolting production features on later** is the
   anti-pattern ADR-0085 rule 2 ("best practices in Rust, now") exists to
   prevent. The handl port (RFC-0005) is only a low-risk *port* if the Rust
   original is already structured the way the handl version will be.
2. **The runner and the framework library are tangled in one crate.** Today
   `plugin-lib-test` hosts *both* the runner (`runner.rs`) and the framework
   library (`assert`/`property`/`benchmark`/`fuzz`/`snapshot` modules + the
   `test.hnl` op wiring). ADR-0092 migrates the framework to handl and
   deletes `plugin-lib-test`; the runner needs a home of its own.

The v0.1 grammar (ADR-0074) has no `test` keyword. Rather than amend the
core grammar (which would bake testing into every handl program), the
`test`/`property`/`integration_test` forms are delivered as a **test
language fragment** merged onto the canonical kit at discovery time, per
ADR-0044's language-as-data model and the existing `Language::merge`
machinery. Production handl code does not see the `test` keyword unless the
test fragment is merged in.

## Decision

**Create a new `plugin-engine-test` crate** as the home of the Rust test
runner, extracting the runner out of `plugin-lib-test`. Architect it as a
**production pipeline** with stable contracts between stages: land the
boundaries now, ship simple initial implementations for each. Future
features become new implementations/compositions, not architectural surgery.

### Crate boundary

| Crate | Role | Fate |
|---|---|---|
| `plugin-engine-test` (**new**) | Test runner: discovery, index, filter, planner, scheduler, engine, result store, summarizer, reporter | Created by this ADR; stays Rust per ADR-0085 rule 6 |
| `plugin-lib-test` | Framework library: `assert`/`property`/`benchmark`/`fuzz`/`snapshot` + `test.hnl` op wiring | Migrated to handl by [ADR-0092](./0092-test-framework-in-handl.md), then **deleted** |

The runner depends on the framework library (tests call `assert.*` etc.),
not the reverse. During migration the runner consumes the framework via the
handl module chain regardless of whether it is the Rust or the handl
implementation.

### Pipeline

```
Providers → Discovery → Index/Cache → Filter → Planner → Scheduler
                                                              ↓
                                                          Engine
                                                              ↓
                                                        ResultStore
                                                              ↓
                                                       Summarizer
                                                              ↓
                                                         Reporter
                                                        ↙      ↘
                                                   Console    Machine
                                                              (JSON/JUnit/...)
```

Every arrow is a contract. No stage reaches across to another stage's
internals; in particular **the engine never prints** and **the reporter
never executes**.

### Stable core types (intentionally hard to change, versioned)

```
Test          — identity + metadata + executable arrow
TestSuite     — hierarchical grouping of Tests
TestMetadata  — path, tags, kind, capabilities, timeout, retries, ...
TestPlan      — selected tests
ExecutionPlan — scheduled tests (parallelism, deps, ordering, resources)
TestResult    — Passed | Failed | Errored | Skipped | Cached
TestReport    — results + summary, structured
TestSummary   — counts, durations, slowest, parallelism
TestError     — location, assertion, expected/actual, diff, cause, duration
TestPredicate — composable filter predicate
```

Providers generate `Test`s; the runner never cares where they came from.

### Test declaration syntax via a merged language fragment

The `test`/`property`/`integration_test` keyword forms are **not** added to
ADR-0074's core grammar. They are a **test language fragment** — a YAML
`Language` fragment with `AstShapeDef`s (declaring roles like `name`,
`body`), `GrammarRow`s, `LexerPattern`s, and `EvalSchemeDef`s — merged onto
the canonical kit at discovery time via `Language::merge` (ADR-0044).

```
canonical_kit.runtime().lang()  ⊕  test_language_fragment  →  test_lang
```

- Production handl code is parsed with the **canonical kit only** — no
  `test` keyword is visible. Testing is a composable concern, not a
  baked-in keyword.
- Test sources are parsed with **canonical ⊕ test fragment**.
- Discovery resolves `test`-kind node children via **named roles**
  (`lang.child_role_index("test", "body")`), never `kind == "test"` shape
  checks (ADR-0081). Adding a new test-kind is a fragment edit, not a
  runner edit.
- The existing `test_*` naming convention is retired as the primary
  discovery mechanism; a `NamingConventionProvider` may remain as a
  backwards-compat shim only if migration requires it.

> **Prerequisite:** author the test-language fragment YAML and wire
> `Language::merge` in the runner's discovery path. This is a data artifact
> + runner wiring change, **not** a grammar amendment to ADR-0074.

### Providers (extensible from day one)

```
TestProvider  →  discover  →  Test[]
```

Built-in providers:

| Provider | Input |
|---|---|
| `UnitProvider` | `test "name" { body }` blocks |
| `PropertyProvider` | `property "name" { ... }` blocks |
| `DoctestProvider` | doc examples in module comments |
| `BenchmarkProvider` | `bench "name"` blocks (collected, not run under `hnl test`) |
| `IntegrationProvider` | `integration_test "name" { ... }` blocks (effect capability) |
| `CompileProvider` | compile-only checks |

New providers do not require modifying the runner.

### Discovery → Index

Discovery produces a **test index**, not execution. This makes `hnl test
--list` and IDE "discover without running" cheap and side-effect-free. The
index carries full `TestMetadata` (canonical path, tags, kind, declared
capabilities, source span).

### Filtering as a predicate model

```
path("str/**") && tag("unit") && !tag("slow")
```

The **predicate combinators** (`&&`, `!`, `||`, glob/path matching) are a
**generic concern** — there is no generic predicate/filter crate in the
platform today. The runner defines the test-specific *fields* (`tag`,
`path`, `kind`, `status`) on top of generic combinator infrastructure.
See **Generic infrastructure dependencies** below: a small generic
predicate crate is extracted, with the runner as first consumer (IDE
symbol filtering, lint rule selection, etc. follow the same shape).

### Planner + Scheduler (separated; scheduler is generic)

```
Tests → Planner (test-specific) → ExecutionPlan → Scheduler (generic) → Workers
```

The **planner** is test-specific: it expresses test dependencies, sharding,
retry policy, ordering, and isolation in an `ExecutionPlan`. The
**scheduler** is a **generic platform system**, not a runner invention:
a bounded worker pool that executes a plan of independent jobs under a
policy. The runner is a *consumer* of the scheduler, contributing only the
`SchedulePolicy` (parallelism, timeouts) derived from the plan.

> The platform already has `plugin-lib-concurrent::Scheduler`, but that is
> a **cooperative-concurrency primitive** (`spawn`/`await`/`yield`/`cancel`
> for the `concurrent.*` stdlib module), not a generic "run N independent
> jobs in parallel under a policy" scheduler. The runner needs the latter.
> See **Generic infrastructure dependencies** below: the bounded-job
> scheduler is a generic system to extract/generalize, with the test runner
> as its first consumer.

### Pure engine by default; capability model is generic

The default engine is the **pure HNL runtime** (tree-walk first; bytecode
transparently) — no filesystem, network, process, environment mutation, or
uncontrolled global state. Scheduler guarantee:

> Pure tests are independently executable unless their metadata declares
> otherwise.

…so aggressive parallelism is safe by construction. **Capabilities are not
a runner concept** — they are a **generic, language-level concept** owned
by `handl-language::capabilities` (ADR-0044 §7): a `Language`'s capability
set is the transitive closure of `Foreign(name)` sources across its eval
schemes, derivable as a static data query (`capabilities(&lang)`). The
runner does **not** define a `Pure`/`Filesystem`/`Network`/`Process` enum;
it **queries** the capability set of the (merged) test language and maps
the result to scheduling decisions:

```
lang.capabilities() is empty              → freely parallelizable
lang.capabilities() ∋ "fs" / "net" / ...  → isolated/constrained resource
```

Future engines (`SandboxEngine`, `ProcessEngine`, `RemoteEngine`) implement
the same `TestEngine` contract; the test model does not change.

### ResultStore (explicit abstraction)

```
Engine → ResultStore
```

Initial implementation: in-memory. The abstraction is what later enables
file-backed, cached, streaming, and remote result stores without touching
the engine or reporter.

### Cache domains (test-specific vs generic)

```
DiscoveryCache     — source → test metadata                  (test-specific, runner)
CompilationCache   — test/module fingerprint → compiled form  (test-specific, runner)
ExecutionCache     — test + runtime + deps → result           (GENERIC: plugin-exec-cache)
```

`DiscoveryCache` and `CompilationCache` are test-specific and live in the
runner. `ExecutionCache` is a **generic eval-result memoisation** concern
already addressed by `plugin-exec-cache::CacheControl`, which fingerprints
ASTs by `(kind, payload)` and short-circuits eval on hit. The runner does
**not** reinvent fingerprinting; it composes `CacheControl` (or a
test-flavoured wrapper that adds test+runtime+deps to the key) onto the
eval engine. Result caching is **opt-in initially**; the fingerprint model
is proven before it broadens. For pure tests,
`same test + same source/deps + same runtime = same result` makes result
caching extremely safe.

> See **Generic infrastructure dependencies** below: `plugin-exec-cache`
> is the generic home; the runner is a consumer, and may justify extending
> its key beyond pure AST identity to include runtime/deps fingerprint.

### Determinism (hard invariant)

Parallel execution may change **timing**, never **logical result
ordering**. Results are re-collected into canonical order before reporting,
so the console output is deterministic regardless of `--jobs`.

### Reporting pipeline (structured until the very end)

```
Result[] → Summary → Report → Formatter
```

`ConsoleReporter`, `JsonReporter`, `JunitReporter`, `TapReporter`,
`CiReporter`, custom — all replaceable. `--report json` / `--report junit`
change no execution.

### Structured failures (never just `false`)

```
Failure {
    test, location, assertion, expected, actual, diff, error, cause, duration
}
```

Console renders a human report; JSON exposes the exact same structured
information. No information is lost crossing the report boundary.

### Observability: structured events from day one

```
RunStarted, RunFinished, SuiteStarted, SuiteFinished,
TestDiscovered, TestSelected, TestStarted, TestFinished,
TestCached, TestSkipped, TestFailed
```

Consumers: console, IDE, CI, progress UI, telemetry, debugging.

### Versioned internal contracts

```
TestDescriptor v1, TestResult v1, Report v1,
Provider API v1, Reporter API v1
```

So third-party providers/reporters don't break on internal changes.

### Engine genericity (ADR-0081)

All AST child positions resolved via `Language::child_role_index`; never
hardcoded `kind ==` checks. The `test`/`property`/`integration_test`
shapes declare roles in the test-language fragment's `AstShapeDef`s.

### Generic infrastructure dependencies

Several pipeline stages are **generic platform systems**, not test-runner
inventions. The runner is a *consumer* of these; the concepts in the
sections above are the runner's consumer-side glue (policies, test-specific
fields, composition). The principle: **if another tool would want it, it
does not belong in `plugin-engine-test`.**

| Generic system | Home | State | Runner contributes |
|---|---|---|---|
| **Capability reasoning** (what can a `Language` touch?) | `handl-language::capabilities` (`capabilities(&lang)`, `foreign_names`, `subset_of`) | **Exists**, language-level (ADR-0044 §7) | nothing — queries it; maps result → scheduling decision |
| **Eval-result memoisation** (ExecutionCache) | `plugin-exec-cache::CacheControl` (AST-fingerprint keyed `ExecControl` rule) | **Exists**, generic; may need key extension to include runtime/deps fingerprint | composes it onto the eval engine; test-specific DiscoveryCache/CompilationCache stay in the runner |
| **Bounded-job scheduler** (run N independent jobs under a policy) | `plugin-lib-scheduler` (`BoundedScheduler`, `SchedulePolicy`) | **Exists**, generic; first consumer is the test runner; future build systems, batch linters, etc. follow | `SchedulePolicy` derived from the test-specific `ExecutionPlan` |
| **Predicate combinators** (`&&`, `!`, `||`, glob/path matching) | `plugin-lib-predicate` (`Predicate<T>`, `StringMatcher`, glob/path matching) | **Exists**, generic; first consumer is the test runner; IDE symbol filtering, lint rule selection, etc. share the shape | test-specific fields (`tag`, `path`, `kind`, `status`) on top |
| **Event bus** (structured events, multiple subscribers) | none today as a generic crate | **Candidate for extracting** if a second consumer appears (LSP, build progress) | the test event vocabulary (`RunStarted`, `TestFinished`, …) |

The runner consumes the existing generic crates listed above. For future
"candidate" generic concerns (such as the event bus), the runner does **not**
block: it can ship a simple in-crate implementation and extract the generic
crate when a second consumer appears (ADR-0085 rule 6: improve in place,
extract when justified). The contracts in the runner are written so extraction
is a move, not a redesign.

### CLI surface

```
hnl test [paths...]                -- run all discovered tests
hnl test --list                    -- list, do not execute
hnl test --filter <predicate>      -- composable predicate
hnl test --tag <tag>               -- tag shorthand
hnl test --jobs N | auto           -- scheduler parallelism
hnl test --report console|json|junit
hnl test --changed                 -- changed-only (filter + cache)
hnl test --cache                   -- opt-in result caching
```

Exit codes (per ADR-0085 / `host-cli`): `0` success, `1` test/parse
failure, `2` usage/parse error.

### API surface

```rust
// Discovery
pub fn discover(paths: &[PathBuf]) -> Result<TestIndex, DiscoverError>;

// Filter + plan
pub fn filter(index: &TestIndex, predicate: &TestPredicate) -> TestPlan;
pub fn plan(selected: &TestPlan, policy: &SchedulePolicy) -> ExecutionPlan;

// Run + report
pub fn run(plan: &ExecutionPlan) -> RunReport;
pub fn run_with_filter(plan: &TestPlan, filter: &TestPredicate) -> RunReport;
pub fn format_report_human(report: &RunReport) -> String;
pub fn format_report_json(report: &RunReport) -> String;
```

The existing `discover`/`run`/`run_with_filter` names are preserved as thin
wrappers over the new pipeline for callers.

### Testing

100% coverage (ADR-0085 rule 4). The crate's own tests double as
conformance tests for the `test` declaration syntax via the test-language
fragment; the stdlib's test suites are integration fixtures.

## Consequences

- **The runner gets its own crate** (`plugin-engine-test`), cleanly
  separated from the framework library. ADR-0092 can delete `plugin-lib-test`
  without touching the runner.
- **The runner is production-architected, not a throwaway.** RFC-0005
  becomes a port of already-generic, well-bounded Rust code — the lowest-
  risk kind of port per ADR-0085 rule 6.
- **Testing is a composable language concern, not a baked-in keyword.**
  The `test` keyword lives in a mergeable fragment (ADR-0044); production
  handl code stays clean. A new test kind is a fragment edit + a provider.
- **Parallelism, caching, new providers, new reporters, new engines** are
  all new implementations behind existing contracts — no architectural
  surgery to add them.
- **Generic infrastructure is reused, not reinvented.** The runner queries
  `handl-language::capabilities` for capability reasoning, composes
  `plugin-exec-cache::CacheControl` for eval memoisation, and consumes the
  generic bounded-job scheduler (`plugin-lib-scheduler`) and predicate
  combinators (`plugin-lib-predicate`). The runner owns only test-specific glue.
- **Determinism is guaranteed** regardless of `--jobs`.
- **CI and IDE share one predicate/filter/index model.**
- **The `test_*` naming convention is retired** as the primary discovery
  mechanism.
- **The test-language fragment becomes a load-bearing artifact.** Its
  `AstShapeDef` roles are the contract the runner depends on; changing
  them is a coordinated edit, not a silent one.

## Prerequisites

1. **Create `crates/plugin-engine-test`** and extract `runner.rs` (and
   runner-only helpers) out of `plugin-lib-test`. The framework-library
   modules stay in `plugin-lib-test` pending ADR-0092.
2. **Author the test-language fragment** (YAML): `test`, `property`,
   `integration_test`, `bench` `AstShapeDef`s with roles (`name`, `body`),
   plus their `GrammarRow`s, `LexerPattern`s, and `EvalSchemeDef`s.
3. **Wire `Language::merge`** in the runner's discovery path to merge the
   fragment onto the canonical kit.
4. **Consume the generic-system crates** (see *Generic infrastructure
   dependencies*): reuse `handl-language::capabilities` and
   `plugin-exec-cache` as-is; consume `plugin-lib-scheduler` for the
   bounded-job scheduler and `plugin-lib-predicate` for predicate
   combinators.
5. ADR-0074 grammar is **not** amended.

## Status

Rewritten. The original minimal-runner text is superseded by this
production-architecture specification for the new `plugin-engine-test`
crate. The framework-library migration is
[ADR-0092](./0092-test-framework-in-handl.md); the future handl port of
this runner is [RFC-0005](../rfcs/0005-test-runner-in-handl.md).
