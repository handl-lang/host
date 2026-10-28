# lib-json-handl dogfood report (round 3)

**Deliverable**: `/home/luis/GitHub/handl-lang/lib-json-handl/src/json.hnl` — a pure-handl
JSON parser + serializer (~880 lines) exercising `data`, `match`, `while`-state
protocols, `IO<sig>`/`handle`, and the approved State/IO fold design — plus this gap
report. Round 3 ran against a rebuilt `libhnl_ext_typecheck.so` (mid-session engine
changes), which *tightened* checking inside `while` bodies and produced a whole new
failure surface. Round-2 findings that still reproduce are carried at the end.

## Verification results

| Command | Result |
|---|---|
| `hnl typecheck src/json.hnl` | **clean** — full record type returned |
| `hnl run src/json.hnl` | **evaluates the whole module**; smoke block = 39-item array, every item verified |
| `hnl test src/json.hnl` | 0 tests — test files can't import the lib (see G-test) |
| `./ci.sh` | fails early: `no hnl binary at '../host-cli/target/debug/hnl'` — gate hardcodes a different path than `~/.cargo/shared-target/debug/hnl` |

**Smoke coverage (all verified in `module.smoke`)**: toStr on every variant + nested
containers; `pretty` (indent, empty containers); `parse` on scalars/arrays/objects/
deep nesting/whitespace, `\n`/`\uXXXX` escapes; `parse` errors for bad literal,
unterminated string, unclosed container, stray `}`/`]`/wrong-kind close, trailing
value, empty/ws-only input, missing comma, trailing comma, `{,}`; `jfirst_scalar`
checked recursion; `run_parse` → `[Ok, 3]` and `[Err@3]`; `runState_c(demo_chain,5)`
→ `[6,6]` (put payload computed *inside* the fold); `runState_c(p_getc,{pos,src})`
→ `["h",{pos:1,src:"hi"}]` — the real `{pos,src}` parser state threaded through the
chain interpreter.

## Gaps — severity ranked

### S1 — Blockers (impossible or unsafe without workarounds)

1. **`type.unknown` is untouchable — not a top type, not a bottom type.**
   Every host-call result and every `while` result is `type.unknown`. On the current
   checker it unifies with *nothing* except itself: `x == 0`, `x == "a"`, `x >= 0`,
   `x + 1`, `if x`, `x && y` all fail ("value Number(0) does not match expected type
   type.unknown", "cannot unify — expected Str, got Unknown"). `unknown == unknown`
   *does* work. Consequences written into the lib:
   - char classes can't be `str.find(…) >= 0` — they're 26-arm `match`es on literal
     char patterns (`class_alpha`, `class_num`, `hex_d`).
   - `while` arithmetic can't touch host results — the tokenizer became a
     char-by-char state machine; positions counted in meta-typed state fields, never
     measured from `char.length`/`slen` results.
   - every shared record field that can carry a produced value must be unknown-typed
     *on every write* (`uw_s`/`uw_n`/`uw_r`/`uw_b` literal-forcers, `noval` sentinel
     instead of `null` since `record == null` and `unknown == null` both fail).
   - "is this field absent" tests must be `match f { null: …, _: … }` — `f != null`
     fails (unknown vs Null).

2. **Closed-record unification kills heterogeneous payloads in shared slots.**
   `match` arms returning different ctor-record shapes fail
   `cannot unify closed records with different fields: ["b"] vs []` — and the same
   check reaches *inside* record fields and array elements. Every `mk_*` mint is
   while-wrapped so results are `type.unknown` and unify; all stack frames are one
   shape `{k,items,key,ph}`, all emit actions `{k,x,s,ind}`, all tokens `{k,v,p}`;
   "absent" is `noval` (a sentinel ctor with a unique sym), never `null`.

3. **Record/array literals crash in `type.unknown`-expected positions.**
   `check_object_record: expected type has no 'fields' object` when a record literal
   lands in array-spread or call-arg position where the expected type resolved to
   `type.unknown`. Fix: mint records inside `{out:}` of a trivial while
   (`mk_tok`/`mk_act`/`mk_frm` in json.hnl — the literal record inside `{out:}`
   synthesizes fine and returns unknown).

4. **`check_json_value: expected type is not a tagged type value` — internal
   crash, several triggers:**
   - passing a *concrete* value to an arrow whose param resolved to `type.unknown`
     (`esc(pk)` where `pk` destructures a typed `[Str, Json]` pair → concrete `Str`
     arg vs `unknown` param — crashes; while-wrapping the bind makes it unknown → OK);
   - a *literal* arg against a *dead* meta param (`mk_jnull` originally took an
     unused `u` — `mk_jnull(null)` crashed; `mk_jnull` is now a bare `:=` def);
   - record arg vs a param already pinned to a *different* record shape
     (`uw_r({pos,src})` after `uw_r` pinned on the noval record);
   - `handle io with` on a runtime-minted IO record at the call site.
   Same underlying bug: the checker serializes an expected type (meta / `type.unknown`
   / mismatched record) that isn't a valid tagged type value → internal error instead
   of a unification diagnostic.

5. **`hnl run`'s integrated checker diverges from `hnl typecheck`.** The file
   typechecks clean but `run`'s check phase raised `check_json_value` crashes at
   positions `typecheck` accepted — mostly calls to unknown-param arrows *outside*
   `while` islands and `do`-block literal positions. The smoke that survived:
   a top-level `smoke_all := [ exprs… ]` of call expressions (elements evaluate for
   output; element type `type.unknown`). Anything more structured (assertion arrows,
   `do`-binds containing calls, nested list literals) tripped the run-checker.
   There is no `hnl run --unchecked` escape hatch (round-2 G15 still stands).

6. **`str.replace` is broken.** `str.replace("a\nb", "\n", "\\n")` returns
   `"a\n\b"` (a + LF + `\` + `b`): the pattern never matches and a `\` is inserted
   before the last char. `str.replace("q\"z", "\"", "\\\"")` → `"q\"\z"` — same
   shape. Arg order is documented as (haystack, pattern, repl); observable behavior
   matches none of the permutations. The lib's `esc` was rewritten around it.

7. **Char-pattern `match` + `str.concat` emits escapes one char late.** The
   `match`-based `esc` emits `\` before the *next* char rather than escaping the
   matched one: `esc("a\nb")` → `a` + LF + `\` + `b` (LF unescaped, `\` before 'b');
   `esc("q\"z\\")` → `q`,`"`,`\`,`z`,`\`,`\` (quote unescaped). Yet `esc("q\"z")`
   → `q\"z` correctly. Control-char arms appear to fire for the *following*
   position — an engine-level `match`-bind/`char.chars` bug; 3 smoke items for
   `\n`-escapes were dropped pending a fix (parse-side escape *decoding* works —
   `\u0041` → `A` verified).
   *Repro in `/tmp/hnl-dogfood3/pe2.hnl`, `pe4.hnl`.*

8. **Postfix `[i]` binds to the arrow name, not the call result.**
   `run_parse("[7]", 0)[0]` evaluates the *arrow* `run_parse` at index 0 →
   `arrow-value expects 2 args, got 1`. Bind first: `r := run_parse("[7]",0); r[0]`.
   Affects every `f(args)[i]`/`f(args).field[i]` shape; indexing *bound* names works.

9. **`data` ctors are file-local at runtime, and `hnl test` can't touch `data`
   even transitively.** `import`ing `json.hnl` from a test file fails desugaring
   the imported `data` (`desugar.data … match: no arm matched`); ctor *names* don't
   cross `import` either (`call_host: function 'JNum' not found`). The lib ships
   `mk_*` mints (identical `__ctor__`/`type` shapes — `match` dispatches on name
   identity, so `mk_jnum(1)` and `JNum(1)` are interchangeable) and documents that
   consumers redeclare `data Json` (`sym.seeded` handles are content-stable).
   There is no usable test harness for this lib — smoke is `module.smoke`.

10. **`IO<sig>` can't be constructed in checked code, and `handle` isn't a
    sequential interpreter.** `IO` values exist only via the runtime spelling
    `{"__former__": sym.seeded("type.io"), "indices": [sig]}` (lib-type's
    `export IO` doesn't materialize: `call_host: 't.IO' not found`). `handle`
    dispatches each sig field independently — no result feeds back into the
    computation — so the approved `State(s) := {get: Unit ~> s, put: s ~> Unit}`
    record encoding is a *summary* only (get sees initial state; put's payload
    *is* the final state). Sequenced get/put requires the free-monad chain:
    `IO({get: k})`, `IO({put: [s', k]})`, `IO({ret: a})` folded by `runState_c`.
    Both are implemented and verified (`run_parse` → `[Ok,3]`; `demo_chain`
    → `[6,6]`; `p_getc` threads real `{pos,src}` → `["h",{pos:1}]`). The approved
    record-shaped `State` cannot express `put(get+1)` — this is a *design* gap in
    the IO former, not just a missing feature: reified op-sets lose ordering and
    result-feedback; chain encoding is the honest fit.

### S2 — Structural friction (workable, costly)

11. **`:=` arrow params are monomorphic across call sites.** A shared `unk`/`uw`
    wrapper pins its param at the first call — the lib needs `uw_s`/`uw_n`/`uw_r`/
    `uw_b` (one per arg type), plus `mk_tok`/`mk_act`/`mk_frm` per record shape.
    Same mechanism that makes `mk_io` unsafe for two sig shapes (parse_st inlines
    its `{get,put,result}` literal instead).

12. **`while` bodies are a *partial* escape, not a full island.** Round 2 treated
    `while` as unchecked; round 3's checker enforces the `type.unknown` rules *inside*
    closures too — every workaround above applies inside them. `do`-blocks inside
    `while` propagate expected types downward, which is what makes literals in
    `{out: do{…}}` positions hit `check_json_value`.

13. **String `+` is Number-only** (`"a"+"b"` → `expected Number, got Str`) — use
    `str.concat` (verified working). All `+` on strings in the lib were converted.

14. **`export` is dead/unsafe in-module** (duplicate binding / unbound behavior);
    named `import x` needs `export` decls → only `import *` / `import * as j` work.
    No visibility control: every top-level `:=` is public.

15. **Recursion needs `: T =`; mutual recursion impossible.** `jfirst_scalar :
    Json ~> Json = …` is the pure-canonical proof (runtime warns
    `lower_ast unsupported: self-recursive call — falling back to AST eval`).

16. **Tuple/pair indexing**: `kv[1]` on typed tuples fails `array_element_type`;
    `match p { [a, b]: … }` destructure is the workaround (used for kv pairs).
    `char.chars(…)[pos]` is unreliable too — `char.at(src, pos)` used in `p_getc`.

17. **Brace-count fragility.** A stray `}` doesn't error at the site — it silently
    re-nests the rest of the file and surfaces as `unbound wire` on unrelated names
    tens of lines later (cost real debugging time: a `build_step` brace made
    `chain_*`/`sig`/`zz` read unbound). Error positions in `run` are byte offsets,
    not line:col, and often point at whitespace/comments.

18. **Number formatting**: `char.length`, positions etc. print as floats (`3.0`);
    `typeof.to_string(3)` gives `"3"`. Parse positions in `PErr` are float-valued.

### S3 — DX / cosmetic

19. `()` isn't a Unit literal (`child_ref` bounds/internal error) — use `null`.
20. `J` is the eliminator keyword — `data J ::` parse-errors ("unexpected token:
    token.bind"); family names must avoid `J`.
21. `Unknown` in an annotation degrades the arrow to `LamVal` → `callee is not
    applicable` when called.
22. `hnl test` reports `0 tests` for `test_*`-free files but can't even *import*
    a module containing `data` — test infrastructure is unreachable for this lib.
23. `ci.sh` hardcodes `../host-cli/target/debug/hnl` — the working binary is
    `~/.cargo/shared-target/debug/hnl`; `scripts/hnl-gate.sh`'s canonical-file
    detector may also skip `~>`-dialect sources.
24. `match` on `type.unknown` scrutinee *works* (list destructuring, ctor-name
    patterns) — one of the few reliable escape valves; document it as the
    intended "untyped" lane.

## Design verdict on the approved State/IO (option A)

The record-shaped `IO<{get, put, result}>` encoding *runs* — `run_parse` returns
`[result, final_state]` — but only as a commutative summary: `handle`'s per-entry
dispatch and the reified op-record both discard ordering and result-feedback.
The chain (free-monad) encoding is the faithful State interpreter and is fully
working: `runState_c` folds `{get: k}`/`{put: [s,k]}`/`{ret: a}` and `demo_chain`
proves `put` payloads computed from earlier `get` results. **Recommendation for
the language:** if `IO` is meant to host real effect interpreters, the former needs
either (a) a sequencing primitive (`bind`/continuation slot in the op record the
runtime knows to fold), or (b) `handle` semantics where op *results* are passed
back into the still-running computation — otherwise every real effect will be
encoded as an explicit chain exactly like `runState_c`.

## Library surface (public API)

`parse : Str ~> Result(PErr, Json)` (mk_-mints; `Err` carries `PErr(pos,msg)`),
`toStr` (compact), `pretty` (2-space), `mk_jnull`/`mk_jbool`/`mk_jnum`/`mk_jstr`/
`mk_jarr`/`mk_jobj`, `mk_ok`/`mk_err`/`mk_perr`, `jfirst_scalar : Json ~> Json`,
`runState`/`runState_c`/`parse_st`/`run_parse`, `chain_get`/`chain_put`/`chain_ret`,
`demo_chain`, `p_getc`, `handled`, `mk_io`. Arrays as `[Json]`, objects as
`[[Str, Json]]` (lib-collections representation). `data Json`/`PErr`/`Result`
declared for the checked surface; `mk_*` mints are the import-safe constructors.
