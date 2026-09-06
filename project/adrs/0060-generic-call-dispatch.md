# ADR-0060: Generic Call Dispatch

## Status

Proposed

Migrates the `call` prim's hardcoded dispatch logic to data, per the
direction established by [ADR-0058 — Generic Data-Driven
Engines](./0058-generic-data-driven-engines.md) §"Existing hardcoded
logic is debt". Follows the template of [ADR-0059 — Generic Object Key
Evaluation](./0059-generic-object-key-evaluation.md), which migrated
object-key resolution from three Rust engines into one desugar rule.
Depends on the arrow-calculus `Scheme` kernel from [ADR-0052 — Arrow
Calculus Kernel](./0052-arrow-calculus-kernel.md) and the desugar pass
from [ADR-0053 — Generic Pratt Parser](./0053-generic-pratt-parser.md)
§7.

## Context

ADR-0058 established that eval engines should be fixed interpreters of
`Language.eval` data. Dispatch is already data-driven: `eval.yaml`
maps the `call` node kind to `Arr { op: "call", source: Native }`. But
the **native op implementation** contains hardcoded language-specific
logic in all three engines (`plugin-engine-exec-tree-walk`,
`plugin-engine-exec-bytecode`, `plugin-engine-exec-cranelift`).

Each engine's `call` handler (`prim_call` / `prim_v2_call`) and its
`apply_app` helper hardcode three concerns that should be separable:

1. **`json{...}(args)` template substitution** — when the callee is a
   `json` node, the engine substitutes `$0`, `$1`, … into the JSON
   template string before parsing. This is sugar: it rewrites the AST
   shape before evaluation. It is the direct analogue of ADR-0057's
   bracket-key form — a surface construct implemented as an engine
   branch instead of a desugar rule.

2. **Callee dispatch** — the two-way decision on the callee's runtime
   value, hardcoded inside `apply_app`:
   - If the callee is a **string**, look it up in `ModuleLookup` and
     call the host `Function`.
   - If the callee is an **object with `__arrow__`**, extract
     `params`/`body`/`env`, build a fresh `ScopeStack` from `env`, bind
     `params`, and evaluate `body`.

   This is language-specific: a different language might dispatch on a
   different marker, support multiple callee shapes, or resolve string
   callees through a different registry. Today this requires editing
   all three engines.

3. **Variable arity** — `call` is variable-arity (one child per
   comma-separated argument). The fixed-arity `Scheme::App` combinator
   cannot express this from a single per-node-kind scheme, which is
   why `call` currently lowers to a native op that reads the node's
   children directly.

ADR-0058 named `call` as deferred debt; this ADR pays that debt by
separating the concerns and moving each to its correct home.

## Decision

Split `call` into three layers, each in its correct home.

### Layer 1: `json{...}(args)` becomes a desugar rule

The `json` callee with arguments is syntactic sugar: it rewrites the
AST before evaluation. Add a desugar rule on the `call` AST shape that
detects a `json` callee with arguments and rewrites it into the
template-substituted form. After desugar, no engine's `call` prim ever
sees a `json` callee.

This is the direct analogue of ADR-0059's bracket-key desugar: a
surface construct that was an engine branch becomes a data rule in
`ast.yaml`, interpreted by the generic `desugar_pass.rs`.

### Layer 2: Variable arity becomes a desugar-time pack

The `call` node's children are `[callee, arg1, arg2, …]` — one callee
plus N args, where N varies per call site. The `eval.call` scheme is
per-node-kind and cannot express variable N with fixed-arity
`proj_child` sub-schemes.

Solution: the desugar pass packs the argument children into a single
`array` child. After desugar, every `call` node has exactly two
children: `[callee, array { children: [arg1, …, argN] }]`. The scheme
uses `proj_child(0)` for the callee and `proj_child(1)` for the args
array. Both `call_host` and `Scheme::App` receive the args as a single
array value and unpack it internally.

This is a structural rewrite, not a semantic change — the same args are
passed to the same callee; only their packaging changes.

### Layer 3: Callee dispatch becomes a scheme description

With `json` handled by desugar and variable arity handled by arg-
packing, the remaining `call` logic is the two-way callee dispatch.
This is language-specific and belongs in `eval.yaml` as a `Scheme`
description using existing kernel combinators.

The `eval.call` scheme becomes a `Seq` of a type-discrimination step
and a `Choice`:

```yaml
- id: eval.call
  node_kind: call
  scheme: !Seq
    fst: !Arr
      op: route_on_type
      args:
        - !Arr { op: proj_child, args: [], source: Native, lit: 0 }
      source: Native
      lit: "string"
    snd: !Choice
      l: !Arr # string callee → host function
        op: call_host
        args:
          - !Arr { op: proj_child, args: [], source: Native, lit: 0 }
          - !Arr { op: proj_child, args: [], source: Native, lit: 1 }
        source: DynamicForeign
      r: !App # non-string callee → closure application
        callee: !Arr { op: proj_child, args: [], source: Native, lit: 0 }
        args:
          - !Arr { op: proj_child, args: [], source: Native, lit: 1 }
```

`route_on_type` takes a value and a literal type tag, returns
`Either::Left(value)` if the value's type matches, `Either::Right(value)`
otherwise. `Choice` routes `Left` to `call_host`, `Right` to `App`.

A different language can now change callee dispatch by editing
`eval.yaml`: add a `Choice` arm for a module-qualified callee, change
the type tag, reorder the arms, etc. — no engine changes.

### What stays in Rust

Per ADR-0058, engine code changes are acceptable only for **new
combinator primitives** (genuinely new kernel operations), not for
surface syntax or new forms of existing primitives. This ADR adds:

- **`typeof`** — a native op under `arr` that returns a type tag string
  (`"string"`, `"object"`, `"number"`, `"bool"`, `"array"`, `"null"`)
  for a given value. A general-purpose type-discrimination primitive,
  not specific to `call`. Used by `route_on_type` and available for
  any scheme that needs to dispatch on value type.

- **`route_on_type`** — a native op under `arr` that takes a value and
  a literal type tag, returns `Either::Left(value)` or
  `Either::Right(value)`. This is the bridge between `typeof` and
  `Choice`: it produces the `Either` that `Choice` routes on. It is a
  utility, not language-specific.

- **`call_host`** — a native op that takes a string callee and an args
  array, looks up the name in `ModuleLookup`, and invokes the host
  `Function` with the array elements as individual arguments. This is
  the host-function half of the current `apply_app`. It uses a new
  `ArrSource::DynamicForeign` (see below) so the capability system can
  audit it structurally.

- **`ArrSource::DynamicForeign`** — a new variant of `ArrSource`
  (ADR-0052) for native ops that perform host-function dispatch with a
  runtime-determined name. The existing `Foreign(String)` variant
  carries the function name at scheme-construction time, enabling
  load-time capability audit. `DynamicForeign` signals that the
  function name is determined at runtime — the capability system can
  flag it, require a blanket grant, or apply a different audit policy.
  This preserves the structural capability boundary: the typechecker
  and load-time audit match on `source`, not on a string prefix.

`Scheme::App` (ArrowApply, ADR-0052) handles closure application. No
new closure primitive is needed — `App` is the kernel combinator for
dynamic application of an arrow received as runtime data, and that is
exactly what closure application is. The `App` interpreter's
`apply_app` helper is modified to accept a single args-array value and
unpack it into individual params (the desugar packing from Layer 2
ensures args always arrive as an array). The string-callee host-
dispatch branch is removed from `apply_app` — that logic moves to
`call_host`.

### What is removed from Rust

The three engines' `prim_call` / `prim_v2_call` functions are removed
entirely — the `eval.call` scheme no longer references a `call` native
op. The `apply_app` / `apply_closure_value` helpers lose their
hardcoded callee-dispatch `if let` chains (string → host, `__arrow__`
→ closure) and the `json{...}` template-substitution branch. They
become simple closure-application helpers that `Scheme::App` calls.

## Consequences

- **One scheme, not three dispatch chains.** Callee dispatch becomes
  one `Choice` in `eval.yaml`, not three Rust `if let` chains. Adding
  a new callee shape (module-qualified, multi-method, etc.) is a data
  change.
- **`json{...}(args)` is sugar, not a special case.** It becomes a
  desugar rule, like bracket keys before it. The engines never see it.
- **No new closure primitive.** `Scheme::App` (ArrowApply) handles
  closure application. The env-swap/param-bind/eval-body protocol
  stays in the `App` interpreter, where it already lives. The ADR
  does not invent a new concept for something the kernel already
  expresses.
- **`typeof` is general-purpose.** Unlike a narrow `is_string`
  predicate, `typeof` returns a type tag usable by any scheme that
  needs value-type discrimination — not just `call`.
- **Capability boundary is preserved.** `ArrSource::DynamicForeign`
  makes dynamic host dispatch visible to the capability system. The
  current `call` op hides it behind `ArrSource::Native`; this ADR
  makes it honest. The typechecker and load-time audit can match on
  `DynamicForeign` and apply an appropriate policy.
- **`Choice` gets real exercise.** The `Scheme::Choice` combinator
  (ADR-0052) has existed in the kernel but had few real uses. `call`
  dispatch is its first significant application — routing on callee
  type at eval time, driven by data.
- **Variable arity is handled by desugar.** The `call` node's
  variable-arity argument list is packed into a single array child by
  the desugar pass. The scheme uses fixed-arity `proj_child` — no
  special-case variable-arity logic in the op.
- **Tests are preserved.** The existing `call`/`foreign`/closure tests
  (`app_calls_closure`, `foreign_dispatches_to_registered_host`,
  `cross_engine_foreign_results_match`, etc.) become the conformance
  baseline. The behavior they assert — string callee → host function,
  closure callee → env-swap + param-bind + eval body — is preserved
  exactly; only its location changes.
- **ADR-0058's `call` debt is paid.** ADR-0058 §"Existing hardcoded
  logic is debt" named `call` alongside `var` and `object`. Object
  keys were paid by ADR-0059; `var` is already a generic `proj_env`
  kernel op. This ADR pays the `call` debt. The remaining ADR-0058
  debt is pattern matching (`try_pattern` in `match`/`handle`), which
  is not addressed here and remains deferred.
