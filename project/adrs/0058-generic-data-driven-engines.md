# ADR-0058: Generic Data-Driven Engines

## Status

Accepted

Generalizes the design principle established by
[ADR-0052 — Arrow Calculus Kernel](./0052-arrow-calculus-kernel.md) (eval is
a fixed interpreter of scheme data) and [ADR-0053 — Generic Pratt
Parser](./0053-generic-pratt-parser.md) (parse is a fixed interpreter of
grammar data). Motivated by the implementation experience of
[ADR-0057 — Computed Object Keys](./0057-computed-object-keys.md), which
exposed the gap between the principle and the current implementation.

## Context

The platform's stated design principle (ADR-0052 §1, ADR-0053 §1) is that
language rules should be **data**, not engine code. A host should be able to
change the language — syntax, semantics, key forms — by editing kit data
(YAML), without touching Rust.

The **parser** achieves this. `PrattEngine` is a fixed interpreter of
`Language.grammar` data. Adding a new operator, keyword, or delimited form
is a grammar row in `grammar.yaml` — no parser code changes.

The **desugar pass** (ADR-0053 §7) achieves this. `AstDesugarRule` is data
on `AstShapeDef`; the pass is a fixed `Ast → Ast` interpreter. The
`if → match` rewrite is declared in data, not code.

The **eval engines do not fully achieve this.** While *dispatch* is
data-driven (the `eval.object` scheme in `eval.yaml` says "call the `object`
native op"), the **native op implementations** contain hardcoded
language-specific logic. Examples:

- **Object key resolution** — the `object` prim has a Rust `match` on key
  node kind: `var` → use name, `string` → use literal, `array` → evaluate
  inner (ADR-0057), else → evaluate and stringify. Adding ADR-0057's
  bracket computed keys required editing all three engines (tree-walk,
  bytecode, cranelift) in lockstep.
- **`var` lookup** — the `var` op has hardcoded scope-chain logic.
- **`call` dispatch** — the `call` op has hardcoded closure-application
  logic, including special cases for string-named host functions.

Each of these is a place where a language change requires engine code
changes, violating the principle.

## Decision

### Principle: engines are fixed interpreters of data

An eval engine should be a **fixed loop** that:

1. Looks up a node's eval scheme from `Language.eval` (already data-driven).
2. Interprets the scheme's `Arr`/`Seq`/`First`/`App` combinators (already
   generic).
3. Dispatches native ops to **data-specified behavior**, not hardcoded
   Rust `match` arms.

Any logic that depends on *which language* is being evaluated (key rules,
special forms, syntactic sugar) belongs in **desugar rules** (data) or
**scheme descriptions** (data), not in engine code.

### Concrete: move object-key resolution to desugar

As the first application, move the object-key rule from the three engines'
`object` prim into a desugar rule on the `entry` AST shape:

```
entry { children: [array { children: [expr] }, value] }
  →  entry { children: [expr, value] }
```

After desugar, no entry key is ever an `array` node. The engine's `object`
prim reduces to two cases: `var`/`string` → use payload (literal key), else
→ evaluate (computed key). The ADR-0057 per-engine `array`-key branches are
removed.

### Going forward: no new language-specific logic in engines

New language features that require per-node-kind behavior must be expressed
as:

- **Desugar rules** (for syntactic rewrites — `[k]:` → computed key,
  `if` → `match`, future sugar).
- **Scheme descriptions** (for eval-time behavior — new native ops are
  declared in `eval.yaml` with their `Arr`/`Seq`/`First`/`App` structure).

Engine code changes are acceptable only for **new combinator primitives**
(genuinely new kernel operations), not for new surface syntax or new
key/forms of existing primitives.

## Consequences

- **One rule, not three.** Object-key resolution becomes one desugar rule,
  not three Rust `match` arms. Adding ADR-0057's bracket keys would have
  been a data-only change.
- **Engines converge on a fixed core.** Each engine implements the kernel
  combinators (`Arr`, `Seq`, `First`, `App`, `Loop`) and a small set of
  native ops. Language-specific behavior migrates to data.
- **Existing hardcoded logic is debt — now paid.** The `var`, `call`,
  `object`, and pattern-matching prims each contained language-specific
  logic that violated the principle. All four have since been migrated:
  `var` is a generic `proj_env` kernel op; object keys migrated to a
  desugar rule ([ADR-0059](./0059-generic-object-key-evaluation.md));
  `call` dispatch migrated to a `Choice` scheme
  ([ADR-0060](./0060-generic-call-dispatch.md)); pattern matching was
  de-duplicated into a shared module
  ([ADR-0061](./0061-generic-pattern-match.md)). The direction this ADR
  established is now fully realized.
- **No runtime cost.** Desugar runs once between parse and eval; the
  engines consume a simpler AST.
- **ADR-0057's implementation is superseded.** ADR-0057's *language design*
  (bracket computed keys are valid) stands; its per-engine Rust
  implementation is replaced by the desugar rule.
- **Future key forms are data.** Template-literal keys, symbol keys, etc.
  become desugar rules on the `entry` shape — no engine changes.
