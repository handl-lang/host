# handl runnable examples

Small, self-contained `.hnl` programs you can run right now with the `hnl` CLI.
Every file here is a single expression and has been verified to run against the
current build. All examples use `~>` arrow notation, `while`, `if`, `let ... in`
bindings, and state spread — the v0.1 canonical arrow-calculus authoring surface
(ADR-0074).

## Run them

```sh
hnl run hello.hnl          # run a file
hnl eval '1 + 2 * 3'       # evaluate an expression inline
hnl fmt factorial.hnl      # pretty-print a file
```

`hnl run` / `hnl eval` use the canonical kit directly, so no `.handl.toml` is
needed. Output is printed as JSON (bare strings are printed without quotes).

```sh
# run every example at once
for f in *.hnl; do echo "=== $f"; hnl run "$f"; echo; done
```

## The examples

### Language core

| File              | Shows                                                         | Output |
| ----------------- | ------------------------------------------------------------- | ------ |
| `hello.hnl`       | The built-in `hello.greet` library call                       | `Hello, world!` |
| `arithmetic.hnl`  | Operator precedence, grouping, `**`, `/`, `%`, `<=>`, unary `-` | object |
| `records.hnl`     | Nested records/arrays, dot access, computed fields with `proc` | object |
| `match.hnl`       | `match` over literals, strings, bools, binders, wildcards, nested arms | object |
| `destructure.hnl` | Array (`[h, ...t]`, `[]`) and object (`{ name: n }`) destructuring in `match` | object |
| `conditionals.hnl`| Branching via `if/else` and nested `match` | object |

### Arrows: `~>` and `>>>`

| File              | Shows                                                         | Output |
| ----------------- | ------------------------------------------------------------- | ------ |
| `closures.hnl`    | `~>` with multi-params, immediate invocation, higher-order combinators (`apply_twice`, `compose`, `thrice`), arrays of closures indexed by position | object |
| `composition.hnl` | `>>>` arrow composition — the kernel's `Seq` combinator as surface syntax | `11` |

### Recursion via `while` (feedback)

ADR-0074 specifies `while` as the v0.1 surface for feedback — the kernel's
`Loop` combinator (ADR-0052). The body closure receives the current state and
returns either `{ out: v }` (terminate with `v`) or `{ state: v }` (continue
with new state `v`).

With **param destructuring**, the body closure can name individual state fields
as parameters instead of accessing them through `s.field`:

```hnl
while { n: 5, acc: 1 } (
  [n, acc] ~> if n == 0 { out: acc } else { state: { n: n - 1, acc: acc * n } }
)
```

| File                | Shows                                          | Output |
| ------------------- | ---------------------------------------------- | ------ |
| `factorial.hnl`     | `while` with destructured accumulator — factorial of 5 | `120`  |
| `fibonacci.hnl`     | `while` with 3 destructured vars — Fibonacci(10) | `55`   |
| `power.hnl`         | `while` with 3 destructured vars — 2^10          | `1024` |
| `sum-to-n.hnl`      | `while` destructured accumulator — sum of 1..100 | `5050` |
| `ackermann.hnl`     | Self-applying closure (Ackermann needs a stack, not a simple loop) | `9` |

### Algorithms

| File                | Algorithm                                         | Output |
| ------------------- | ------------------------------------------------- | ------ |
| `gcd.hnl`           | Euclidean GCD with destructured params — gcd(48, 36) | `12`   |
| `is-prime.hnl`      | Trial division with `if/else if` chain — is 29 prime? | `true` |
| `largest-factor.hnl`| Largest prime factor by trial division — of 84    | `7`    |
| `collatz.hnl`       | Collatz step count with `if/else if` — from 27    | `111`  |
| `fast-exp.hnl`      | Binary exponentiation with destructured params — 3^13 | `1594323` |
| `binomial.hnl`      | Binomial coefficient with 4 destructured params — C(10, 3) | `120`  |
| `binary-search.hnl` | Binary search using `let ... in` bindings + state spread | `6`    |
| `popcount.hnl`      | Count set bits with destructured params — popcount(255) | `8`    |
| `digital-root.hnl`  | Digital root with nested `if` — of 942            | `6`    |
| `tribonacci.hnl`    | Tribonacci with 4 destructured params — T(10)     | `149`  |
| `hanoi.hnl`         | Tower of Hanoi move count — 3 disks               | `7`    |
| `fizzbuzz.hnl`      | FizzBuzz via `if/else if` chain — last value (15) | `15`   |

### List processing

Lists are processed by destructuring `[h, ...t]` in a `while` that carries the
remaining list and an accumulator in the state.

| File                | Shows                                          | Output |
| ------------------- | ---------------------------------------------- | ------ |
| `list-length.hnl`   | Count via head/tail in `while` + `if`          | `5`    |
| `list-sum.hnl`      | Reduce/fold with an accumulator in `while`     | `15`   |
| `list-max.hnl`      | Track-a-best with `if` inside `match`          | `9`    |
| `list-contains.hnl` | Linear search, early exit via `{ out: true }`  | `true` |

## Patterns to steal

**`while` with param destructuring** — the body closure names state fields
directly, no `s.` prefix needed:

```hnl
while { n: 5, acc: 1 } (
  [n, acc] ~> if n == 0 { out: acc } else { state: { n: n - 1, acc: acc * n } }
)
```

**`if` expression** — desugars to `match cond { true: a, _: b }`:

```hnl
if n == 0 { out: acc }
else if n % 2 == 0 { state: { ... } }
else { state: { ... } }
```

**`let x = y in body` binding** — desugars to `(proc (x) -> body)(y)`, lets you
name intermediate values:

```hnl
let mid = (s.lo + s.hi) / 2 in
let guess = s.xs.mid in
if guess == s.target { out: mid } else { ... }
```

**State spread** — `{ ...s, field: val }` copies all fields from `s`, then
overrides specific ones (later entries win):

```hnl
{ state: { ...s, lo: mid + 1 } }    -- carry all state, change only lo
```

**Arrow composition** (`>>>` — the kernel's `Seq` combinator):

```hnl
5
  >>> (proc (x) -> x + 1)
  >>> (proc (y) -> y * 2)
```

**Self-applying `proc`** (for non-tail-recursive patterns like Ackermann):

```hnl
(proc (self) -> self(self, 2, 3))(proc (self, m, n) -> match m { 0: n + 1, _: ... })
```

**Higher-order** `proc` combinators:

```hnl
(proc (f, g, x) -> f(g(x)))(proc (n) -> n + 1, proc (n) -> n * 2, 5)   # compose -> 11
```

## Current limitations (canonical kit via `hnl run`/`eval`)

These work in the richer plugin-loaded kits (see `lib-utils/` and the
`crates/plugin-lib-*/examples/` corpora) but **not** in the bare canonical kit
that `hnl run`/`hnl eval` use today:

- `#` comments, assignment statements (`x = y` without `in`)
- The arrow kernel combinators `first`/`choice`/`app` are not callable as
  surface keywords. `~>` (arrow construction), `>>>` (composition), and
  `while` (feedback) are wired. `while` uses the kernel `Loop` step protocol
  (`{ out: v }` / `{ state: v }`) via `loop_step` (ADR-0052).
- Array spread of a *computed* value (`[h, ...self(self, t)]`) — spread of a
  literal (`[1, ...[2, 3]]`) is accepted by the parser but does not flatten
  computed results, so list-building combinators like `map`/`reverse` that
  return new arrays are not yet practical. The examples here stick to
  scalar-returning folds (`sum`, `max`, `length`, `contains`).
- Bracket indexing (`xs[0]`); use dot-index instead (`xs.0`)
- String concatenation (`"a" + "b"`) — `+` only works on numbers.
- `while` runs in all three eval engines (tree-walk, bytecode, cranelift).
  All four derived step ops (`loop_step`, `while_step`, `for_step`,
  `fix_step` from `plugin-lib-arrow-iter`) are supported across all three
  engines.
- The library modules (`str.*`, `math.*`, `json.*`, `http_server.*`, mood
  operators `is`/`may`/`must`/`when`, etc.) — these require a kit loaded via
  `.handl.toml` through the plugin host.

For the full module-rich examples (crypto, http server, json/toml/yaml, mood
type operators, etc.), see `lib-utils/` and
`crates/plugin-lib-*/examples/<module>/`.
