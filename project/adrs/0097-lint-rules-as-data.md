# ADR-0097: Lint Rules as Language Data

## Context

ADR-0044 ("language as data") established that engines and tooling are
generic and data-driven: a new language is a YAML fragment merged into a
`Language`, never an engine recompile. The formatter (`plugin-engine-fmt`)
and typechecker (`plugin-engine-typecheck` + `*-nbe`) already follow this —
the fmt/typecheck/test split is generic engine + canonical data.

The linter did not. `plugin-ext-lint` hardcoded all eight canonical rules in
Rust (`rule_unused_import`, `rule_unbound_wire`, …), with a bespoke
`LangInfo` classifier and a token-position zipping hack to recover spans the
Pratt AST already carries. ADR-0094's `lint` pass (`find_matches` over
`TransformDef`s) proved a lint rule can be _data_: `simplify-paren` was
already declared in `plugin-ext-lint/data/lint_transforms.yaml` — but only
as a pattern matcher, with the diagnostic and fix still computed in Rust.

Meanwhile `plugin-engine-lint` (the generic AST lint engine) and
`plugin-ext-lint` had grown into parallel implementations that never met:
the engine's `LintEngine::lint` served embedded hosts (`host-tooling`,
`plugin-diag-expression`) while the ext crate reimplemented its own walk.

## Decision

Lint rules are first-class `Language` data.

1. **`LintRuleDef` fragment** (`handl-language::fragments::lint_rule`),
   merged into a new `Language::lints: FragmentSet<LintRuleDef>` (added with
   `serde(default)` so pre-ADR-0097 serialized languages still load).
   Fields: `id` (fragment identity), `name` (the reported rule name —
   several fragments may share a name as alternative arms of one logical
   rule), `severity` (`"error"|"warning"|"info"`), `message` (a `{key}`
   template), `check`, optional `fix`, and the usual `replaces`/`requires`.

2. **Two check kinds**, mirroring ADR-0094's declarative/native split:

   - `check: !Match { node_kind, when? }` — node-local matching. `when` is a
     `TransformCondition`, evaluated by the _same_ evaluator the rewrite
     engine uses (`plugin-engine-transform-rewrite::condition::matches_condition`,
     now public). The predicate vocabulary gains
     `!ChildPayloadEq { index?, role?, path?, value }` — "the child at
     role/index has payload `value` at `path`" — needed to test a child
     node's payload (e.g. a constant `true`/`false` condition).
   - `check: !Native "name"` — the disciplined escape hatch, resolved
     against the lint engine's `NativeLintRegistry`. Natives receive a
     `LintCtx` (language, source, parsed statements, shared
     `ModuleAnalysis`) and return `NativeHit`s (template args + absolute
     span + optional fix). An unregistered native name is a `LangError` at
     lint time — a declared rule with no implementation is a configuration
     bug, not a pass.

   `fix:` on a `Match` rule is declarative (`!Unwrap { role|index }` —
   replace the node's source text with its child's); `Native` rules compute
   their own fixes.

3. **Module-level engine API** in `plugin-engine-lint`:
   `parse_module` (the bootstrap one-statement-per-parse splitting
   convention, moved out of `plugin-ext-lint`), `LintModule`,
   `ModuleAnalysis::build` (per-statement bindings / references / arrow
   params / imports, classified via `AstShapeDef` roles and `pattern_role`,
   spans taken from `AstNode::span` + statement offset), and
   `LintEngine::lint_module` / `lint_source`. `LintDiagnostic` gains `rule`
   and `fix`. The single-AST `LintEngine::lint` built-ins are unchanged.

4. **Canonical natives** ship in `plugin-engine-lint`
   (`canonical_lint_natives()`, wired via
   `LintEngine::with_canonical_lint_natives` — the analogue of
   `with_desugar_natives`): `scope.unbound-wire`, `scope.shadowed-binding`,
   `scope.unused-import`, `naming-convention`,
   `source.missing-module-doc`. `source.fmt` is registered by
   `plugin-ext-lint` — it needs `plugin-ext-fmt`, and engine crates do not
   depend on ext crates.

5. **Canonical rules live in `handl-kit-data-canonical/data/lints.yaml`**,
   merged by `merge_fragments` (10th fragment file) and `language_from_dir`
   (optional, like `transforms.yaml`). `plugin-ext-lint` slims to language
   assembly, engine wiring, `plugin-ext-diag` conversion, and the fix
   fixpoint (~70% of the crate deleted); `plugin-ext-diag::Diagnostic::rule`
   widened from `&'static str` to `String` because rule names are now data.

   The `lint`-pass `TransformDef` in `plugin-ext-lint/data/lint_transforms.yaml`
   was later _removed_: `host-lsp` builds the `simplify-paren` quick fix
   directly from the diagnostic's `fix` payload (`LintFixOut` →
   `plugin-ext-diag::Fix`), so the transform-engine `lint`/`lsp-rewrite`
   passes had no remaining consumer. The match predicate now lives solely
   in `lints.yaml` as the `LintRuleDef`.

## Consequences

- Adding or tuning a canonical lint rule is a `lints.yaml` edit — no
  recompile of the rule logic, no `plugin-ext-lint` release. Third-party
  languages contribute rules through their own fragments.
- Span quality improved for free: `unbound-wire`, `shadowed-binding`, and
  `naming-convention` now report real `AstNode::span` positions (the old
  token-position zip emitted degenerate `0:0`/`1:1` spans). Verified by
  byte-identical diagnostic _sets_ over `handl-packages/` + `examples/`
  (5,189 diagnostics, positions normalized) against the pre-change binary.
- `!ChildPayloadEq` is shared vocabulary: `validate_transforms` checks its
  role/index bounds at `Language` build like every other condition.
- A language can declare `!Native "…"` rules its host must register; the
  canonical `scope.*` set documents the boundary between data and Rust.
- `.hnl`-authored lint natives landed: the same `handl:<mod>.<fn>`
  spelling ADR-0094 added for transforms works for `!Native` lint checks.
  `NativeLintRegistry` gained a generic `NativeLintResolver` fallback seam
  (`add_resolver`, consulted on map miss — `plugin-engine-lint` stays
  runtime-free); `handl_plugin_lib_handl::lint::HandlNativeLint` is the
  adapter mirroring `HandlNativeRewrite`: it holds `HandlModule`s, resolves
  `handl:` names, and invokes the function **once per module** with a
  `{source, statements: [{text, offset, root}]}` record (node records use
  the transform shape `{kind, span, payload, children}` but with spans
  absolutized). The function returns `null`/`[]` or a list of hit records
  `{name?, args?, span?, fix?}` mapped to `NativeHit` (fixes included).
  Module-level rather than per-node was chosen deliberately: `!Native`
  carries no `node_kind` filter, so a per-node contract could express
  nothing `!Match { node_kind, when }` cannot — the point of the seam is
  the module view `!Match` cannot reach. Like `HandlNativeRewrite`, no
  shipped host wires it yet (canonical `lints.yaml` declares no `handl:`
  rules); hosts opt in via `reg.add_resolver(HandlNativeLint::new()
.with_module(...).resolver())`. See `plugin-lib-handl/tests/handl_lint.rs`.
- Done: the remaining built-in engine checks (`empty-block`,
  `unreachable-code`, `shadowed-binding`, `unused-binding`) are now declared
  as `!Native "builtin.*"` rules in `crates/handl-kit-data-canonical/data/lints.yaml`
  and implemented in the canonical lint native registry.

## Status

accepted
