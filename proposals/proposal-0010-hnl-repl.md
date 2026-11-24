+++
id = "P-010"
title = "hnl repl"
status = "open"
created = "2026-11-23"
priority = "low"
labels = ["cli", "repl"]
+++

# P-010 — hnl repl

***Status:** draft · **Priority:** low · **New repo:** `ext-repl` ·
**Task:** T-140
**Builds on:** `ext-eval` eval path (`Kit::eval`/`Runtime::eval`),
`tool-parse` completeness detection

## Idea

`hnl repl` — a proper interactive loop for the language, packaged as an
`ext-*` cdylib like every other `hnl` command (ADR-0149). Not a lib:
the REPL *is* a host tool; `lib-repl` (T-141) stays parked until an
app-level consumer exists — this dogfoods the pattern first.

```text
$ hnl repl
handl repl — :help for commands
> x := 40 + 2
> x * 2
84
> deploy({ env: "staging" })
{ status: "ok", env: "staging" }
> :load scripts/setup.hnl
> :q
```

## Command surface

- `hnl repl [--kit <dir>] [--engine <name>]` — same kit/engine flags
  as `hnl eval`; `--engine` selects the eval backend per session.
- Dot-commands (colon-prefixed, never evaluated):
  - `:q` / `:quit` — exit 0
  - `:help` — command list + a one-line language cheat sheet
  - `:load <file>` — evaluate a file into the session env
  - `:defs` / `:env` — list names bound in the session
  - `:type <expr>` — run the typechecker on an expression, print sig
  - `:engine <name>` — switch eval backend mid-session (if cheap)

## Core mechanics

1. **Line accumulation** — buffer lines until `tool-parse` accepts
   the source or fails with a *non*-EOF error (report + reset buffer).
   EOF-kind parse failure = incomplete input → `...` continuation
   prompt, Python-REPL style.
2. **Session env** — the hard part: `Kit::eval` today evaluates a
   module fresh per call. REPL needs a *persistent* environment —
   defs from line N visible on line N+1. Likely needs an engine-side
   `Session`/`Env` handle (eval-with-env-in, env-out) rather than
   re-evaluating accumulated source each line (O(n²) and wrong for
   effectful defs). **Open question:** does `Runtime`/`Kit` already
   expose resumable eval? If not, smallest kernel addition: an
   `eval_session` op threading an env record.
3. **Value rendering** — result → display string. Reuse whatever
   `hnl eval` prints today; if it's debug-format only, a `show`/`repr`
   pass is follow-up work, not blocking.
4. **IO** — stdin `read_line` + stdout `print` via `lib-stdio`/posix
   ffi (the `~[1]~>` edge stays in Rust ext code, not `.hnl`).

## Non-goals v1

- Line editing (readline/killer-fish), history persistence, syntax
  highlighting — cooked-mode stdin only; a `rustyline`-style backend
  is a later `lib-repl-c`-style option.
- Multi-line *editing* (up-arrow recall) — v1 is forward-only.
- Debugging/inspection beyond `:type`/`:defs`.

## Tests

- Scripted stdin → stdout golden tests (`printf '1 + 2\n:defs\n:q\n' |
  hnl repl`) — the smoke-suite `.out` convention applies.
- Continuation-prompt cases: unclosed `{`, `match`, multi-line `do`.
- `:load` into session; env persistence across lines; error recovery
  (bad line → prompt returns, env intact).
- `--engine` selection parity with `hnl eval`.

## Open questions

- Session env: existing `Runtime` support vs kernel addition —
  investigate `exec-*`/`kit-prelude` eval entry points first.
- Does value printing need a dedicated `show` op for records/arrows?
