# Language as data

On the handl platform, **a language is a value**. The tokens, grammar
rules, AST shapes, eval schemes, and type rules that define a language are
ordinary data — YAML fragments merged into a `Language` object — not code
compiled into an engine.

This is the platform's second big idea after the closed kernel
([ADR-0044](https://github.com/handl-lang/host/blob/project/adrs/0044-language-as-data-and-runtime-split.md),
"language as data and runtime split").

## The `Language` value

The `handl-language` crate (in the [`kernel`
repo](https://github.com/handl-lang/kernel)) defines the schema. A
`Language` is assembled by merging fragment tables:

| fragment | what it declares |
| --- | --- |
| `tokens` | the token vocabulary and its tooling categories |
| `lexer_rules` | how source text becomes tokens |
| `grammar` | how tokens become AST — binding powers, child shapes |
| `ast` | the shape of each node kind — named child roles, arity |
| `eval` | how each node kind becomes a `Term` in the calculus |
| `types` | the typing schemes per node kind |
| `transforms` | rewrite and desugar rules |
| `caps`, `capabilities`, `globals`, `options`, `representations`, `value_types` | capability wiring, globals, option schemas, value representations |

None of these fragments is privileged. A host assembles the `Language` it
needs by merging whichever fragments apply; the canonical eval-path
fragment set is published as data in
[`kit-prelude-lang`](https://github.com/handl-lang/kit-prelude-lang).

## Engines dispatch on data

The `engine-*` crates are **generic machinery**: each is an abstraction
crate for one operation — `hnl-engine-lex`, `hnl-engine-parse`,
`hnl-engine-transform`, `hnl-engine-exec` — plus concrete engines like the
tree-walk interpreter (`hnl-engine-exec-tree-walk`). They share one rule:

> Dispatch on node kind via `Language` data — never a hardcoded `match`
> on kind strings.

What is allowed to be fixed in Rust is only the calculus vocabulary: the
combinator set, primitive forms, and value shapes. Adding a language
construct is a YAML fragment change; adding a calculus combinator is an
engine edit. The two almost never happen together.

The same discipline applies to AST layout: engines resolve child
positions by **named role** from the `ast` fragment's `AstShapeDef` (ADR-
0081), not by positional index or kind checks — so renamed or reshaped
languages run on untouched engines.

## What this buys

- **One engine, many languages.** Lexing, parsing, desugaring, and
  evaluation are parameterized by the `Language` value. A DSL, a config
  language, or a language variant is assembled by merging fragments — no
  fork, no recompile of the engine.
- **Per-workspace variants.** Because a `Language` is data, different
  projects can pin different fragment sets and evolve independently on
  the same machinery.
- **Tooling agrees by construction.** A formatter, a linter, an LSP
  server, and an interpreter that consume the same `Language` value
  cannot drift apart — there is one definition.
- **A foreign project can emit language fragments.** Vocabularies are
  data with a stable schema, so external tools can generate rows that the
  engines execute directly.

## Where the boundary sits

The engine layer is deliberately small:

- **Data** — everything specific to a language lives in fragments.
- **Calculus** — the six-combinator vocabulary lives in the
  [kernel](./arrow-calculus-kernel).
- **Engines** — the generic dispatch machinery lives in the `engine-*`
  repos.
- **Host** — composition, kits, options, and capabilities live in the
  [platform](./platform-architecture).

The canonical surface language built on this machinery is not yet
released; the machinery itself is what these docs cover.
