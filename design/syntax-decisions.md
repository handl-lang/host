# Syntax decisions — 2026-11-24 (user-approved)

Captured from design discussion; implementation dispatched to grammar agent.

## Dependent Π — `(x : A) ~> B`

Typed-binder function syntax in *type position* mints the same `type.pi`
that `Pi(A, x ~> B)` produces. `Pi`/`forall`/`Forall` call-forms keep
working (sugar/legacy). `Family`, `Sigma`, `Path`, `PathP`, `Glue`,
`Equiv` stay named formers — genuinely different kinds, not arrows.

Rationale: `Pi(A, x ~> B)` already stores `body_closure{var_name, body_idx}`
— the `x ~> B` arg IS a function node; `(x : A) ~> B` packs domain+binder+
body into one typed binder. Untyped `x ~> B` stays `type.function`;
position (type vs term) disambiguates.

## Dependent pair — `(x : A, y : B(x))`

Parenthesized typed-binder list with NO `~>` = `type.sigma`; non-dependent
`(x : A, y : B)` = product. The `~>` is the function/pair disambiguator.
Matches `[w, v]` pair-literal intro. Dependency flows left-to-right like
`Sigma`'s closure. `Sigma(...)` call-form stays as sugar.

## Arrow taxonomy

| Form | Meaning |
|---|---|
| `A ~> B` | plain arrow |
| `A ~>[n] B` | dimension-n arrow `Hom(n,s,t)` — operator + trailing bracket, consistent with `>>>[n]` |
| `A ~[G]~> B` | graded arrow — bracketed grade infix (repurposes today's `dim_arrow` shape for grades) |

`~>[` vs `~[` lexically distinct. Old `~[n]~>` keeps parsing during
transition (numeric payload → dimension reading; named/expr → grade).
**Codemod later**: mechanical `~[n]~>` → `~>[n]` pass over `.hnl` sources
via the codemod/transform engine (same playbook as `let`→`:=`, smaller
surface). Do not break existing files before the codemod runs.

Reserved: `~[g]~>` generalized graded arrows are vision — grade slot is
deserved now, semantics lands later.

## Kept as-is

- `match v return (i, w) ~> T with` — `return` stays (Agda/Idris spelling,
  clearer than `:`)
- `IO<{ puts, fs }>` — brace-free `IO<puts, fs>` to be supported as sugar
  IF parser allows (agent verifying; cosmetic either way)
