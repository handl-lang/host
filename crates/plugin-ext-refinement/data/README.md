# Arrow DSL language definition (data-driven)

This directory contains YAML data files defining the **arrow DSL**
(ADR-0069 arrow-graph syntax) as a data-driven `Language`, matching the
format used by the main handl language in
`crates/handl-kit-data-canonical/data/`.

## Why a separate language definition?

A feasibility study found that the arrow DSL and the main handl language
share operator glyphs but have **different semantics**:

| Glyph | Arrow DSL meaning        | Main language meaning          |
|-------|--------------------------|--------------------------------|
| `>>>` | `Seq` (arrow composition)| `App` (function application)   |
| `|||` | `Choice`                 | `Choice` (same, but namespace) |

Because `>>>` means `Seq` in the arrow DSL but `App` in the main language,
the arrow DSL cannot reuse the main language's `grammar.yaml` / `eval.yaml`.
Instead, this directory defines a **separate arrow-specific `Language`**
with its own token namespace (`token.arrow.*`), grammar rows, eval schemes,
and AST shapes.

## Files

| File              | Contents                                                      |
|-------------------|---------------------------------------------------------------|
| `tokens.yaml`     | Token kinds (`token.arrow.*`) for keywords, operators, etc.   |
| `lexer_rules.yaml`| Lexer rules matching the tokens (keywords > identifiers in    |
|                   | priority; multi-char operators > single-char).               |
| `grammar.yaml`    | Pratt parser grammar rows (prefix keyword combinators with    |
|                   | delimited arg blocks; infix `>>>` / `|||` / `=`).            |
| `eval.yaml`       | Eval schemes mapping AST node kinds to arrow-calculus         |
|                   | `Scheme` IR (`Seq` / `Choice` / `First` / `Loop` / `Arr` /    |
|                   | `App`).                                                       |
| `ast.yaml`        | AST shape (payload schema) definitions for each node kind.    |

## Relationship to `arrow_parser.rs`

The ad-hoc parser in
`crates/plugin-ext-refinement/src/arrow_parser.rs` currently handles the
arrow DSL by directly constructing `Scheme` IR via a hand-written
recursive-descent parser. These YAML files are the **data-driven
equivalent** of that parser: once wired into the main data-driven parser
infrastructure (`handl-kit-data-canonical`'s `language_from_yaml` path),
the ad-hoc parser can be retired in favor of the Pratt parse engine +
tree-walk eval engine.

This is **definition only** — the files are not yet wired up to the
parser. The integration is a future task.

## Integration notes

### The `$str_child0` convention

The arrow DSL's `arr("sort")` uses the string literal `"sort"` as the
**op name** of the resulting `Scheme::Arr`. The current `Scheme` kernel
has no primitive that extracts a child's payload string into the `op`
field (which is a static `String`, not a `Scheme`). The placeholder
`$str_child0` in `eval.yaml` denotes "the string value carried by child
0's payload, used as the op name." The integration layer will need either:

1. A **parser-time desugar** that extracts the string into the node's
   payload and a new `Scheme` primitive that reads it, or
2. A **specialized eval-engine hook** for `arr_call` / `app_call` /
   `arrow_var` that reads child 0's payload string as the op name.

### Rich loop syntax

The ad-hoc parser also handles a rich `loop { ... } ( proc (state) ->
match ... )` syntax used in the stdlib `.hnl` files, via heuristic
pattern matching in `analyze_branch`. That syntax is **not** covered by
this data-driven definition; it would require additional grammar rows
and eval schemes (or a desugar pass) when integrated.
