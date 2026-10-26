# The type system

Handl's types are **values**, and the checker is a normaliser. There is no
separate type syntax layer: `Nat`, `Vec<3, Bool>` and `Type(0)` are ordinary
values in the same tagged representation as terms, which is why a type may
mention a term and a term may be inspected as a type.

## Unification is a judgment

Structural equality does not decide everything here: index equalities reduce to
arithmetic over symbolic atoms, and some of those are genuinely open. `unify`
therefore answers one of three things.

| answer | meaning | what happens |
|---|---|---|
| `Proven` | equal for every instantiation | the check proceeds |
| `Disproven` | no instantiation makes them equal | type error; in a `match` arm the arm is auto-discharged |
| `Unknown` | outside the solver's fragment | deferred, warned, or left to the caller |

The solver sits behind an `IndexSolver` seam. The built-in one normalises index
atoms to **ring normal form**, so `?k + 1 + m` and `m + ?k + 1` are the same
polynomial, adds a linear/division/root fragment with non-negativity facts, and
reads the constructor index projections each family declares. Unsolved
metavariables participate honestly as ring atoms, which is what lets
`?k + 1 = var{m} + 1` solve by same-operator congruence.

A solver answering `Unknown` is the design, not a failure: it is how the system
stays sound while its decision procedure stays deliberately partial.

## The surface

Arrows are the only callable shape, and they are values:

```
id      : a ~> a
compose : (a ~> b) ~> (b ~> c) ~> (a ~> c)
```

Implicit binders are written `<...>` and come in two kinds that differ in what
survives to runtime — **indices** are erased (signatures are typecheck-owned
metadata), **evidence** is materialised as a real argument:

```
append<n, m, a> : Vec<n, a> ~> Vec<m, a> ~> Vec<n + m, a>
lookup<a, ev : Eq(a)>
```

A family is declared with a **generator record** (ADR-0158): the field's
codomain decides what the field *is*.

```
Nat := { Zero : Nat, Succ : (pred : Nat) ~> Nat }

Vec<n : Nat, a : Type> := {
  VNil  : Vec<0, a>,
  VCons : (x : a, xs : Vec<n, a>) ~> Vec<n + 1, a>
}
```

A field returning the family is a constructor, folded by `match` or `F.rec`; a
field returning anything else is an effect operation, folded by `handle`. That is
one mechanism with two readings — which is why the same record form declares
`State := { get : Unit ~> Nat, put : Nat ~> Unit }` and `IO` becomes an internal
marker rather than a separate surface.

The declaration is also the metadata the checker uses for index projection:
declaring the `Succ` shape makes `Vec(Succ n)` and `Vec(n + 1)` definitional in
both directions.

::: warning Write the result index
A constructor whose result index is left implicit parses, but the checker then
synthesises a *free* metavariable index — and `VCons(Zero, true, VNil)` will
check at `Vec<5, Bool>`. Always state it.
:::

## Existentials

```
one_vec  : Sigma(Nat, m ~> Vec(m, Bool)) = [Succ(Zero), VCons(true, VNil)]
one_vec  : exists m : Nat, Vec(m, Bool)  = [Succ(Zero), VCons(true, VNil)]   -- same type
```

`exists` is a spelling of `Sigma`, introduced by a pair literal whose first
component binds the second's family. The destructured witness binds **rigidly**
(`var{m}` in both results and context, internally renamed so it cannot be
shadowed) — that rigidity is what lets a computing `filter` rebuild its own
result type:

```
filter<n, a> : (a ~> Bool) ~> Vec<n, a> ~> Sigma(Nat, m ~> Vec(m, a))
```

## Coverage

`match` on an indexed family is checked with **split trees**, not a flat
constructor list: each arm refines the scrutinee's indices and the checker
explores the refinements. Two consequences are visible in ordinary code —
decided-impossible arms are discharged rather than rejected, and `absurd` stays
legal documentation without being required. A family the solver cannot decide
produces a *coverage-unknown* warning rather than a silent pass.

## Dimensions

Higher cells are one indexed family, checked generically in the dimension:

```
A ~[2]~> B      -- a 2-cell: an arrow whose endpoints are arrows
>>>[k]          -- composition at dimension k
id_k            -- identity cell at dimension k
```

`Hom(n)` values normalise as composition trees, with associativity, unit
absorption and interchange holding definitionally.

## Effects, instances, and the top

`IO<{ ... }>` makes effects an index, and `handle` is a typing operation on that
index: covering an op **replaces** its row entry with the handler's return type,
leaving a residual signature. `IO<{ puts: Unit, fs }>` means "puts is handled and
returns `Unit`; `fs` is still performed". Open rows unify today; they have no
surface spelling yet.

Instances are values, not interfaces: `instance Eq(Nat) := { eq: ... }` registers
a dictionary, coherence is enforced when the instance is *declared* (overlap is
rejected, not resolved later), and an `ev : Eq(a)` binder receives that
dictionary at runtime.

`type.unknown` is the dynamic top: host results are accepted anywhere, and it is
deliberately opaque to `==`, `<`, `+` and `if`. Passable anywhere, computable
nowhere until narrowed.

## What is not there yet

An SMT backend behind `IndexSolver` (nonlinear and modular index facts stay
`Unknown`), a surface spelling for open effect rows, quotient/higher inductive
types (design note only), and full backend parity for self-recursive arrows. The
normative sources are ADR-0106 with 0152/0153/0154; the phase-4 spec orders what
remains.
