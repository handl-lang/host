# Rust → Handl cheat-sheet

Reference for porting Rust code to idiomatic handl. Lookup tables first,
idiom rules after. For _why_ the seams exist, see ADR-0044/0077/0083/0096.

## What stays Rust

Only the calculus vocabulary and irreducible atoms: combinators, `cap`/`op`
declarations handled by `rust_*` natives, FFI marshalling primitives,
`error.raise`/`error.try`. Everything else — including interfaces to those
atoms — is handl.

## Construct map

| Rust                | Handl                                                                                             | Example                                     |
| ------------------- | ------------------------------------------------------------------------------------------------- | ------------------------------------------- |
| `fn f(x: T) -> U`   | `export f: T ~> U := (x) ~> expr`                                                                 | `export now_ms: NowMs := () ~> do { ... }`  |
| `struct`            | record type                                                                                       | `Timespec := { tv_sec: Int, tv_nsec: Int }` |
| `enum` (ADTs)       | `tag.tag("Name", payload)` + `tag.tag_of`/`untag`/`match_type`; prefer `option`/`result` packages | `tag.tag("Some", { value: v })`             |
| `Option<T>`         | `option` pkg: `option.some(v)` / `option.none()`                                                  | `option.test.hnl`                           |
| `Result<T,E>` / `?` | `result` pkg: `result.ok`/`err`; or `error.raise(v)` for aborts                                   | `{__tag__:"ok", value: v}`                  |
| catch `Err`         | `error.try(f)` → `{__tag__:"ok"\|"err", value}`                                                   | `error.try(() ~> ffi.call(...))`            |
| `match`             | `match x { pat: expr, _: default }` — literals, binders, `[]`/`[h,...t]`, `{field: b}`            | `examples/handl/match.hnl`                  |
| `if/else` stmt      | expression `if c { a } else { b }`                                                                | —                                           |
| `for`/iterators     | `while {state} {step}` — step returns `{state: v}` (continue) or `return v`/`{out: v}` (done)     | `examples/handl/fibonacci.hnl`              |
| `map`/`fold`/…      | `fn.map`, `fn.fold`, `collection.*`, or combinator graph                                          | `unicode-handl.hnl`                         |
| recursion           | `while` (preferred) or self-application `(self ~> self(self, x))`                                 | `examples/handl/destructure.hnl`            |
| `mut` / `&mut`      | explicit state record threaded through `while`/`rec`; no aliasing                                 | `{ xs: xs, acc: acc }`                      |
| `panic!`/`unwrap`   | `error.raise(v)` — never returns                                                                  | `collections/vec.hnl` `pop`                 |
| trait + impls       | contract pkg (`axiom`/`op`/`guarantee:`) + backend pkg per impl                                   | `unicode` → `unicode-handl`                 |
| module/`impl` block | `do { helpers := ...; { exported: fields } }` — module-as-record                                  | `unicode-handl.hnl`                         |
| `cargo test`        | `*.test.hnl` + `hnl test`; `test_name := assert expr`                                             | `option/option.test.hnl`                    |
| dep in Cargo.toml   | `.handl.toml` `[dependencies] name = { path = ... }`                                              | —                                           |
| `dlopen` FFI        | `ffi.call`/`ffi.bind`/`ffi.errno` + `types/c` aliases                                             | `generated/libc/*.hnl`                      |

## Bindings and syntax quick-ref

```hnl
export f: T ~> U := (x) ~> expr   -- top-level, typed
x := expr  /  x :: expr           -- definition (both = assign; := canonical)
x = expr                          -- in do{} statement position: binding;
                                  -- in expression position: equality (==)
do { a = f(); b = g(a); a + b }   -- sequential block; last expr is value
while { n: 5, acc: 1 } {          -- loop: init state record, step arrow
  [n, acc] ~> if n == 0
    return acc
  else { { state: { n: n-1, acc: acc*n } } }
}
arr("list.tail") >>> first(id)    -- combinator style (calculus vocabulary)
rec { ... }                       -- fixpoint region (state machines)
from "m" import { a, b }          -- named
from "m" import * as m            -- namespace → m.f(...)
handle arr f with Mod             -- capability injection (see time-c)
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

## Idiom rules

1. **Contract first.** Declare the interface (`axiom`/`op`/`guarantee:` or
   exported types + `cap`) in a neutral package; backends are
   `name-handl` / `name-c` / `name-native` / `name-stub`. Callers pick the
   backend by import path (`from "time-c" import { now_ms }`).
2. **Fail loudly, never approximate.** If a backend can't implement an op
   correctly, omit it — callers get a loud failure (`unicode-handl` omits
   `nfc`/`nfd` rather than shipping identity).
3. **Semantic names, not implementation names** — `nfc`, never
   `nfc_icu4x`; `graphemes`, never `graphemes_utf16`.
4. **Effects are visible** — FFI arrows carry `~[1]~>`/`FFI` grade; pure
   ones don't.
5. **Prefer `while` state records over self-application recursion** — the
   corpus pattern; reads as the loop's invariant.
6. **Docs are `---` + `@details`/`@param`/`@returns`/`@example`**; every
   package gets a `docs.yaml` (`slug`/`title`/`description`).
7. **Marshal at the edge.** Convert `Bytes`/handles/`{ret,buf,outs}` into
   semantic handl types inside the backend — never leak C shapes upward.

## Port checklist

- [ ] contract package exists or is written (types + laws)
- [ ] backend package `name-c`/`name-handl` exports the contract record
- [ ] `docs.yaml` + `---` headers
- [ ] `*.test.hnl` covering each export; gate FFI-dependent tests on
      probe results (`error.try` around `ffi.call`)
- [ ] `hnl fmt --check && hnl lint && hnl typecheck && hnl test` green
      (`scripts/dogfood-gate.sh`)
