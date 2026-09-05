---
description: "Engines, tooling, and the arrow-calculus kernel must be fully generic, composable, and data-driven — never hardcoded to a specific language."
trigger: always_on
---

# Engine / Kernel / Tooling Genericity

Engines, tooling, and the arrow-calculus kernel are **fully generic,
composable, and data-driven** (ADR-0044 "language as data"). A new language
is a data change (YAML fragments merged into a `Language`), never an engine
recompile.

- Dispatch on node kind via `lang.eval`/`lang.types`/`lang.grammar`/`lang.lexer_rules` — never a Rust `match` on hardcoded kind strings like `"let"`, `"if"`, `"assign"`, `"paren"`, `"block"`, `"return"`, `"type-annot"`.
- AST layout knowledge comes from `lang.ast[node.kind]` (`AstShapeDef`), not from `node.kind == "assign"` shape checks inside primitives.
- Behavior is built by composing arrow-calculus combinators from `Scheme` data, not by adding `if node.kind == X` branches.

**Allowed to be fixed in Rust:** only the calculus vocabulary itself — the combinator set, primitive _forms_ (e.g. `LexerPattern` variants), value shapes. Adding a calculus combinator = engine edit; adding a language construct = YAML fragment.

**If you find a hardcoded kind check:** move that knowledge into `Language` data (`AstShapeDef`/`EvalSchemeDef`/`TypeRuleDef`/primitive registry). Flag residual hardcoding in `plugin-engine-eval-*` and `plugin-engine-typecheck-*` as tech debt to close, not patterns to extend.

## Data-driven AST child roles (ADR-0081)

- Resolve AST **child positions** by **named role** from `AstShapeDef` — `lang.child_role_index(node.kind, "target")` / `AstShapeDef::role_index` — **never** by positional child index or `kind == "assign"/"block"/"paren"/"return"/"type-annot"` shape heuristics.
- Roles are declared as `AstShapeDef.children` in `data/ast.yaml` (e.g. `assign` → `target`/`value`, `block` → `statements`). A new language is a **data change**, not an engine edit: renamed kinds (`set`/`params`/`body`) must run the untouched engines.
- Keeping a `kind ==` shape check in an engine after roles exist = a regression. Use the role; fall back to positional only for shapes that declare no roles.

Refs: ADR-0044, ADR-0052, ADR-0054, ADR-0081, `crates/handl-language/src/language.rs`.
