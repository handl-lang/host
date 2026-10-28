# Rust → Handl cheat-sheet

Reference for porting Rust code to idiomatic handl. Lookup tables first,
idiom rules after. For _why_ the seams exist, see ADR-0044/0077/0083/0096.

## What stays Rust

Only the calculus vocabulary and irreducible atoms: combinators, effect ops
(`cap`/`op` + `handle ... with rust_*` natives — the host/FFI seam, _not_
the abstraction mechanism), FFI marshalling primitives. Everything else —
including interfaces to those atoms — is handl.

## Construct map

| Rust                | Handl                                                                                                   | Example                                     |
| ------------------- | ------------------------------------------------------------------------------------------------------- | ------------------------------------------- |
| `fn f(x: T) -> U`   | `export f: T ~> U := (x) ~> expr`                                                                       | `export now_ms: NowMs := () ~> do { ... }`  |
| `struct`            | record type                                                                                             | `Timespec := { tv_sec: Int, tv_nsec: Int }` |
| `enum` (ADTs)       | `tag.tag("Name", payload)` + `tag.tag_of`/`untag`/`match_type`; prefer `option`/`result` packages       | `tag.tag("Some", { value: v })`             |
| `Option<T>`         | `from "option" import { some, none }` — a handl pkg, must be imported                                   | `option.test.hnl`                           |
| `Result<T,E>` / `?` | `from "result" import { ok, err }` — type+ctors only; combinators (`map`/`and_then`) are a separate pkg | `result.test.hnl`                           |
| exceptions/`catch`  | none — errors are values. Consume via `match tag.tag_of(r) { "ok": …, "err": … }`                       | `tag.untag(r).value`                        |
| `match`             | `match x { pat: expr, _: default }` — literals, binders, `[]`/`[h,...t]`, `{field: b}`                  | `examples/handl/match.hnl`                  |
| `if/else` stmt      | expression `if c { a } else { b }`                                                                      | —                                           |
| `for`/iterators     | `for x in xs { body }` — imperative _surface_, lowers to `rec` in the arrow calculus                    | `datetime/helpers.hnl`                      |
| `map`/`fold`/…      | `fn.map`, `fn.fold`, `collection.*`, or combinator graph                                                | `unicode-handl.hnl`                         |
| `while` loop        | `while {state} {step}` — step returns `{state: v}` (continue) or `return v`/`{out: v}` (done)           | `examples/handl/fibonacci.hnl`              |
| recursion           | `while`/`for` (preferred) or self-application `(self ~> self(self, x))`                                 | `examples/handl/destructure.hnl`            |
| `mut` / `&mut`      | `:=` writeback in `for` bodies; state record threaded through `while`/`rec`; no aliasing                | `{ xs: xs, acc: acc }`                      |
| `panic!`/`unwrap`   | `result.err(msg)` — fallible arrows return `Result`; no raise/throw in handl                            | `generated/libc` adapters                   |
| trait + impls       | type decl + a value of that type — `I := { f: T ~> U }`, `impl: I := { f: ... }`                        | `unicode-handl.hnl`, `time-c`               |
| module/`impl` block | `do { helpers := ...; { exported: fields } }` — module-as-record                                        | `unicode-handl.hnl`                         |
| `cargo test`        | `*.test.hnl` + `hnl test`; `test_name := assert expr`                                                   | `option/option.test.hnl`                    |
| dep in Cargo.toml   | `.handl.toml` `[dependencies] name = { path = ... }`                                                    | —                                           |
| `dlopen` FFI        | `ffi.call`/`ffi.bind`/`ffi.errno` + `types/c` aliases                                                   | `generated/libc/*.hnl`                      |

## Bindings and syntax quick-ref

```hnl
export f: T ~> U := (x) ~> expr   -- top-level, typed
x := expr                         -- definition / binding (top level, do, for-body)
a == b                            -- equality (= / :: assigns are legacy
                                  -- spellings, being removed from grammar)
do { a := f(); b := g(a); a + b } -- sequential block; last expr is value
for x in xs { acc := f(acc, x) }  -- imperative sugar → rec lowering;
                                  -- := writes back to enclosing scope;
                                  -- the for expression itself is []
while { n: 5, acc: 1 } {          -- stateful loop: init record, step arrow
  [n, acc] ~> if n == 0
    return acc
  else { { state: { n: n-1, acc: acc*n } } }
}
arr("list.tail") >>> first(id)    -- combinator style (calculus vocabulary)
rec { ... }                       -- fixpoint region (state machines)
from "m" import { a, b }          -- named
from "m" import * as m            -- namespace → m.f(...)
-- ALWAYS import. Bare `mod.f` resolution (tag, error, str, …) is a
-- legacy fallback slated for removal — corpus uses it, new code must not.
handle arr f with Mod             -- effect injection — host/native ops only
                                  -- (see time-c); NOT the trait/impl idiom
```

## FFI descriptor map (ADR-0083)

| C type                         | Descriptor                                             | Handl arg                         |
| ------------------------------ | ------------------------------------------------------ | --------------------------------- |
| `int32_t`, `int64_t`, `double` | `"i32"` `"i64"` `"f64"`                                | number                            |
| `const char *`                 | `"cstr"`                                               | string (UTF-8, no NUL)            |
| `void *`                       | `"ptr"` / `"handle"`                                   | number / `{__resource__:{ptr:N}}` |
| `const void * + size_t`        | `"in_buf"` / `"cbytes"`                                | Bytes                             |
| `void * + size_t` out          | `"out_buf:N"`                                          | — → `{ret, buf}`                  |
| `void *` out (fixed)           | `"out_ptr:N"`                                          | — → `{ret, buf}`                  |
| `T *` value-result             | `"io_buf:N"` `"io_scalar:T"`                           | — / seed                          |
| `const T *` / `T *`            | `"in_struct:{…}"` `"io_struct:{…}"` `"out_struct:{…}"` | record / —                        |
| `void` ret                     | `"void"`                                               | → null                            |

Bytes are `{"__bytes__": [n, ...]}`; plain arrays also accepted on input.
`errno` flag on `ffi.call` maps negative returns to `errno` extraction —
wrap in `result.err` at the adapter edge (see `generated/libc`).

> **Gap:** `ffi.call` _raises_ on `dlopen`/`dlsym` failure — incompatible
> with the Result-only error model. A non-raising probe op (`ffi.load` /
> `ffi.sym` returning `Result` or `Option`) is needed before handl-side
> library/symbol probing (e.g. ICU version sniffing) can be written
> idiomatically. `error.try` exists as a host atom but is not the idiom.

## Idiom rules

1. **Contract = a type; impl = a value of that type.** Declare the
   interface as a record/arrow type (`I := { f: T ~> U, ... }`, plus
   `axiom`/`guarantee:` laws where refinement matters) in a neutral
   package; a backend (`name-handl` / `name-c` / `name-native` /
   `name-stub`) exports a value of that type — usually a `do { ...;
{ f: impl } }` record. Callers pick the backend by import path
   (`from "time-c" import { now_ms }`). `cap`/`op`/`handle ... with`
   are for _effects_ (host/FFI ops), never for trait-style abstraction.
2. **Errors are values — no raise/throw.** Fallible arrows return
   `Result<T, E>`; `error.raise`/`error.try` are host atoms for runtime
   internals, not the handl idiom (legacy corpus usage like
   `collections/*.hnl` `pop` is slated to move).
3. **Fail loudly, never approximate.** If a backend can't implement an op
   correctly, omit it — callers get a loud failure (`unicode-handl` omits
   `nfc`/`nfd` rather than shipping identity).
4. **Semantic names, not implementation names** — `nfc`, never
   `nfc_icu4x`; `graphemes`, never `graphemes_utf16`.
5. **Effects are inferred, not annotated** — effectfulness comes from the
   `cap`/`op` declaration, not the signature. `~[N]~>` is an arrow
   _dimension_ annotation (ADR-0119), not an effect row — don't write it
   for effect marking.
6. **`for` for iteration, `while` for stateful loops** — `for x in xs` when
   you're walking a list; `while {state} {step}` when the loop is a state
   machine or terminates on a condition. Avoid self-application recursion.
7. **Docs are `---` + `@details`/`@param`/`@returns`/`@example`**
8. **Marshal at the edge.** Convert `Bytes`/handles/`{ret,buf,outs}` into
   semantic handl types inside the backend — never leak C shapes upward.

## Port checklist

- [ ] contract package exists or is written (types + laws)
- [ ] backend package `name-c`/`name-handl` exports the contract record
- [ ] `---` headers + `@details`/`@param`/`@returns`/`@example`
- [ ] `*.test.hnl` covering each export; FFI-dependent tests gate on
      library availability (see FFI gap note)
- [ ] `hnl fmt --check && hnl lint && hnl typecheck && hnl test` green
      (`scripts/dogfood-gate.sh`)
