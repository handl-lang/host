+++
id = "P-012"
title = "lib-proc: process API (argv/exit/pid)"
status = "open"
created = "2026-11-23"
priority = "med"
labels = ["cli", "process", "stdlib"]
+++

# P-012 — lib-proc: process API

***Status:** draft · **New repo:** `lib-proc` · **Backend:** ops row in
`plugin-lib-stdlib` · **Task:** T-146 · **Kit:** `kit-cli-app`

## Why

`lib-cli-router`'s `run(app)` currently takes `argv` explicitly and
returns the exit code with an `opts.exit` hook — because **no
own-process argv or exit primitive exists**. `lib-posix` stays strict
POSIX.1 (argv is not a POSIX function; `getenv` already covers env).
Process identity/launch data is a **host-runtime** concern: the `hnl`
process already has argv — expose it, don't syscall for it.

## Design (port/impl split)

- **`lib-proc`** — contract only: `pkg.hnl` (`name: "proc"`,
  `modules: ["proc"]`), `.hnl` spec declaring signatures + docs.
  No implementation — the ops are atoms.
- **Backend** — `plugin-lib-stdlib`: new `ops/process.rs` +
  `ModuleSpec` row (`proc` module, ops `proc.argv`/`proc.exit`/
  `proc.pid`, `RegistrySource::Host`). Rust impl is trivial:
  `std::env::args()` / `std::process::exit(code)` /
  `std::process::id()`.
- **`kit-cli-app`** — add `"proc"` to `libs` (argv/exit/stdio
  lifecycle belongs to CLI apps). Not `kit-web-server` v1.

## API

```handl
proc.argv()  ~> [String]        -- own-process argv, argv[0] included
proc.exit(n) ~> a               -- terminate with exit code n
proc.pid()   ~> Int             -- own pid
```

## Consumer change

`lib-cli-router`: `run(app)` becomes self-contained —
`run(app, proc.argv())` internally (or `run(app)` defaulting to
`proc.argv()`), `proc.exit(code)` replaces `opts.exit` hook.

## Plan

1. `lib-proc` repo: contract pkg + spec `.hnl` + README (mirror
   lib-posix/lib-mri scaffold).
2. `plugin-lib-stdlib`: `ops/process.rs` + packages.rs row + unit
   tests (argv shape, pid>0; exit tested via subprocess or omitted).
3. `kit-cli-app/kit.hnl`: `+ "proc"`.
4. `lib-cli-router`: wire `proc.argv()`/`proc.exit`, update tests.

## Open

- `proc.env()`? — NO: POSIX `getenv` covers it; keep proc minimal.
- cwd/umask/etc — later if a consumer asks; spec stays tiny.
