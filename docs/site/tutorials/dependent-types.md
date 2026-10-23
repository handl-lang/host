# Dependent types in handl

In this tutorial you will write a **length-indexed vector** in handl —
the canonical dependent-types example — and use the features that make
it work: `data` families, index arithmetic, `match` refinement,
`Path`/`transport`/`J`, `IO(sig)` handlers, and implicit `{…}` binders.

Every step is a small `.hnl` file you can check with `hnl typecheck`
and (where noted) run with `hnl run`. Along the way you will
intentionally break one file to see what a real type error looks like.

## What you will learn

- How to declare an indexed `data` family and its constructors.
- How annotations like `Vec(1 + 1, Bool)` are *computed* at check time.
- How `match` refines the scrutinee's index per branch — and rejects
  unreachable arms.
- How `Path`, `refl`, `p # v` (transport), and `J` work as the
  equality layer.
- How `IO(sig)` types effectful functions and how `handle … with`
  discharges them.
- How `{…}` binders declare implicit type-level parameters.

## Prerequisites

- A working `hnl` binary (see [Set up the platform](./setup)).
- Familiarity with the arrow surface: `x ~> body` is a function,
  `name : Type = value` is an annotated binding.

## Step 1: Declare a `data` family

Create `vec.hnl`:

```hnl
data Vec(n : Nat, a : Type) :: VNil : Vec(0, a) | VCons(x : a, xs : Vec(n, a)) : Vec(n + 1, a)
```

Read it as: `Vec` is a family of types indexed by a natural number
`n` and a payload type `a`. It has two constructors:

- `VNil` — the empty vector, at index `Vec(0, a)`.
- `VCons(x, xs)` — pushes an `a` onto a `Vec(n, a)`, producing a
  `Vec(n + 1, a)`. The result index is an *index term*, not just a
  variable — `n + 1` is evaluated by the checker.

Check it:

```text
$ hnl typecheck vec.hnl
vec.hnl: ok: {"__registry__":"handl.canonical","__tag__":"type.unit"}
```

A `data` declaration produces no value, so the module checks to
`type.unit`.

## Step 2: Construct and annotate values

Append to `vec.hnl`:

```hnl
v : Vec(2, Bool) = VCons(true, VCons(false, VNil))
w : Vec(1 + 1, Bool) = v
```

```text
$ hnl typecheck vec.hnl
vec.hnl: ok: {"__registry__":"handl.canonical","__tag__":"type.unit"}
```

Two things happened:

- `VCons(true, VCons(false, VNil))` synthesizes `Vec(1 + 1, Bool)` —
  `VNil` is `Vec(0, Bool)`, each `VCons` adds one. That is
  *definitionally* equal to `Vec(2, Bool)`, so `v` checks.
- `w`'s annotation computes `1 + 1` to `2`, so it accepts `v`. Index
  arithmetic (`+`, `-`, `*`) is normalized by the checker — `Vec(n+m)`
  and `Vec(m+n)` are the same type.

Constructor values are ordinary values too, so this file also runs:

```text
$ hnl run vec.hnl   # prints the tagged constructor value for `v`/`w`
```

## Step 3: See a real error

Change `v`'s annotation to a wrong length:

```hnl
v : Vec(3, Bool) = VCons(true, VCons(false, VNil))
```

```text
$ hnl typecheck vec.hnl
vec.hnl:2:20: error [typecheck]: cannot unify — expected Vec(3, Bool), got Vec(1 + 1, Bool)
```

The diagnostic points at the annotated expression (`2:20`) and renders
both sides in surface syntax — `Vec(3, Bool)` vs the synthesized
`Vec(1 + 1, Bool)` — not internal encodings. Revert the annotation to
`Vec(2, Bool)` before continuing.

## Step 4: `match` refines the index per branch

Add a `head` binding that only accepts non-empty vectors:

```hnl
head : Bool = match v { VCons(x, xs): x }
```

```text
$ hnl typecheck vec.hnl   # ok
$ hnl run vec.hnl         # prints: true
```

`match` on a family value does **dependent elimination**: in each arm
the scrutinee's index is refined to that constructor's result index.
You can see the flip side by adding the `VNil` arm back:

```hnl
head : Bool = match v { VNil: false, VCons(x, xs): x }
```

```text
vec.hnl:4:8: error [typecheck]: match: unreachable constructor arm (decided index mismatch)
```

`v` is `Vec(2, Bool)` — a `VNil` (`Vec(0)`) arm can never match, so the
arm is rejected as unreachable rather than silently kept. Inside a
function whose index is a *variable* (`n` instead of `2`), both arms
stay reachable and each gets its own refined index: under `VNil` the
checker knows `n ≡ 0`; under `VCons` it knows `n ≡ k + 1` for some `k`.

## Step 5: Motives, `Path`, `refl`, and transport

When the *result type* depends on the scrutinee, give `match` an
explicit **motive** with `return … with`:

```hnl
id_vec : Pi(Nat, n ~> Pi(Vec(n + 0, Bool), v ~> Vec(n + 0, Bool))) =
  n ~> v ~> match v return (i, w) ~> Vec(i, Bool) with {
    VNil: eq_n # v,
    VCons(x, xs): eq_n # v,
  }
```

```text
$ hnl typecheck vec.hnl   # ok
```

- `Pi(A, x ~> B)` is a dependent function type: "for `x : A`, produce a
  `B`". `id_vec` takes `n : Nat`, then a `Vec(n + 0, Bool)`, and
  returns the same type.
- `return (i, w) ~> Vec(i, Bool)` is the motive — the result type,
  parameterized by the refined index `i` and the scrutinee `w`.
- In each arm the checker binds a **`Path` witness** named `eq_<index>`:
  under `VNil`, `eq_n : Path(n, 0)`; under `VCons`,
  `eq_n : Path(n, k + 1)`. `eq_n # v` **transports** `v` along that
  path, rewriting its type to what the motive needs.

`Path(a, b)` is the equality type; `refl` is the proof that `a ≡ b`
when the endpoints decide equal:

```hnl
v : Vec(2, Bool) = VCons(true, VCons(false, VNil))
p : Path(2, 2) = refl
w : Vec(2, Bool) = p # v      -- transport along p
w2 : Vec(2, Bool) = refl # v  -- refl is the identity transport
```

A false equation is rejected:

```hnl
bad : Path(2, 3) = refl
-- error [typecheck]: refl cannot prove Path(Number(2), Number(3)) — decided inequality
```

`J` is the general path eliminator — same job as `p # v`, with the
motive spelled out as an argument instead of `return … with`:

```hnl
id_vec : Pi(Nat, n ~> Pi(Vec(n + 0, Bool), v ~> Vec(n + 0, Bool))) =
  n ~> v ~> match v return (i, w) ~> Vec(i, Bool) with {
    VNil: J eq_n, (i2, w2) ~> Vec(i2, Bool), v,
    VCons(x, xs): J eq_n, (i2, w2) ~> Vec(i2, Bool), v,
  }
```

> **Type-level only.** `Path`, `refl`, `#` transport, and `J` are
> proof machinery — they exist so the *checker* can rewrite types.
> `hnl typecheck` verifies them; `hnl run` does not give them a
> computational meaning (transported values evaluate to their payload).

## Step 6: `IO(sig)` and `handle … with`

`IO` is a former indexed by an **operation signature** — a record
naming the ops a computation may perform:

```hnl
print : String ~> IO({puts: String ~> Unit}) = s ~> print(s)
```

`print` returns an `IO` whose sig declares one op, `puts`, taking a
`String` and returning `Unit`. Signature subtyping is record-width: an
`IO({puts})` value checks wherever a wider `IO({puts, gets})` is
expected — a caller is only *allowed* more ops than the callee uses.

`handle <expr> with { op: impl, … }` discharges ops. A covered op's
sig entry is replaced by the impl's return type:

```hnl
puts_impl : (String ~> Unit) ~> Unit = req ~> req("x")
x : IO({puts: Unit}) = handle print("x") with {puts: puts_impl}
```

Coverage is checked both ways — a handler naming ops the body never
uses, or covering none of them, is an error; uncovered ops pass through
to the residual `IO` for an outer handler. `handle f with h` also works
on an arrow itself (deep handler), producing `A ~> IO(S')`.

> **Type-level note.** The annotations and coverage rules above are
> enforced by `hnl typecheck`. Running a `handle` expression interprets
> the declared ops at runtime — it needs real op implementations, so
> toy handlers like the one above are check-time illustrations.

## Step 7: Implicit `{…}` binders

Prefix a name's binders with `{name}` (optionally `{name : Sort}`) to
make it an **implicit index/type parameter**: callers never pass it;
the checker inserts a fresh metavariable at each application and
unification solves it.

```hnl
id_io{s} : (String ~> IO(s)) ~> String ~> IO(s) = f ~> x ~> f(x)
y : IO({puts: String ~> Unit}) = id_io(print)("x")
```

```text
$ hnl typecheck vec.hnl   # ok
```

`id_io` never mentions `s` in the body — it is the effect row of
whatever function you pass. Applying it to `print` solves
`s := {puts: String ~> Unit}` automatically. `{s : Sig}` declares the
sort explicitly when inference needs a hint.

## What runs vs. what is type-level

| Surface | `hnl typecheck` | `hnl run` |
| ------- | --------------- | --------- |
| `data` families + constructors | checked | constructors are values; `match` on them evaluates |
| Index arithmetic (`Vec(1+1)`) | computed during checking | n/a |
| `match` refinement / unreachable arms | checked per branch | evaluates the matched arm |
| `Path`, `refl`, `p # v`, `J` | checked (proof objects) | type-level only |
| `IO(sig)`, `handle … with` | sig + coverage checked | interprets via real op impls |
| Implicit `{…}` binders | meta inserted + solved | erased |

## Next steps

- [The arrow calculus kernel](../explanation/arrow-calculus-kernel) —
  why `~>` and combinators are the whole computation model.
- [Graded algebraic theory](../explanation/graded-algebraic-theory) —
  the theory behind `IO(sig)` and grades.
- [Reference](../reference/) — judgment ops and type-rule data for
  engine authors.
