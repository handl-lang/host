# handl-plugin-ext-eval

Evaluation extension point for the Handl platform.

This crate does **not** ship a capability plugin. It re-exports the
generic evaluation machinery from `handl-platform` — `BasicEvaluator`,
the `Eval` operation (`EvalRequest` / `EvalBehavior`), `EvalContext`,
`ExecControl`, `Module`/`ModuleLookup`, `Function`, and the error/limit
types — plus function-registration adapters in `src/adapters.rs`:

- `GenericFunction` — a `Function` backed by a closure.
- `LazyFunction` — a `Function` that constructs its inner function on
  first call and caches it.
- `AsyncFn` / `EvalFuture` — `AsyncFunction` (and `Function`) backed by a
  closure returning a boxed future.

These are the ADR-0040 replacements for the retired `plugin-vm-common`
closure adapters. Concrete evaluation rules live in `plugin-lib-*` /
`plugin-eval-*` crates that kits register; the crate exists so hosts and
library authors can wire eval behavior without depending on
`handl-platform` internals beyond the re-exported surface.
