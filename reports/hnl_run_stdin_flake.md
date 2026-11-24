# hnl-run stdin `null` flake — root-cause report

Date: 2026-11-16 (investigation). Binary: `graphopt/target/debug/hnl-run`
(`mlopt-kit` bin, `packages/mlopt-kit/src/bin/hnl-run.rs`).

## Symptom

Once, `hnl-run -` (stdin mode) returned `null` **instantly** on the
llama1 verify program (`/home/luis/tmp/llama1_verify_prog.hnl`, a single
`do { … }` block whose body `tensor.load`s the 700 MB
`llama1_e2e.safetensors` sidecar), while `hnl-run <file>` computed
correctly on every subsequent run.

## Verdict

**Not a race and not stdin-specific.** `hnl-run` has *two*
deterministic paths that print `null` with exit 0 without doing any
model work, and both reproduce instantly. The stdin-vs-file difference
in the incident was almost certainly a difference in the **bytes fed**,
not in the `-` code path — the only thing `-` changes is how `src` is
obtained (`hnl-run.rs:59-67`, `read_to_string` until EOF; no empty-input
guard). File-arg mode read the stripped `do {}` program; whatever went
to stdin was either empty or headed by a statement that evaluates to
`null`.

### Path 1 — empty stdin → `null` (most likely)

```sh
printf '' | hnl-run -        # → "null", exit 0, ~ms
printf '   \n' | hnl-run -   # → "null", exit 0
```

Chain:

- `hnl-run.rs:59-67` — `-`/absent arg → `stdin().read_to_string(&mut s)`;
  EOF with zero bytes yields `s == ""` with no error.
- `tool-parse-pratt/src/lib.rs:774-789` — empty token stream (or
  trivia-only) → `Ok(Ast::new())`, an **empty arena**.
- `exec-tree-walk/src/lib.rs:3914-3916` —
  `if ast_rc.is_empty() { return Ok(TaggedValue::Null); }` →
  `println!("{v}")` prints `null`, exit 0.

So *any* invocation where stdin is already at EOF — `hnl-run -` run
without a pipe, `hnl-run - < /dev/null`, stdin inherited from a harness
that closed it or a script that already consumed it, or a pipeline
producer (`cat`, a Python `communicate`) that wrote nothing — produces
exactly "`null`, instantly, exit 0". `read_to_string` blocks until EOF,
so a *partial* write cannot produce this; only a zero-byte read can.

### Path 2 — only the FIRST top-level statement is parsed/evaluated

This is the subtler, more dangerous finding, and it is **not**
stdin-specific — it applies identically to file mode.

`tool-parse-pratt/src/lib.rs:769-811` (`parse`): after lexing the
*whole* source, the parser calls `expression_inner` **once** and returns
the arena. Everything after the first top-level statement is lexed
(lex errors still fire — `1+1\n@@@` → `LexError`) but never parsed or
evaluated:

```text
$ printf '1 + 1\n2 + 2\n'        | hnl-run -   → 2     (first wins)
$ printf '2 + 2\n1 + 1\n'        | hnl-run -   → 4
$ printf '1 + 1\ntensor.load("/nonexistent","x")\n' | hnl-run - → 2, no error
$ printf '1 + 1\n)\n'            | hnl-run -   → 2     (trailing garbage unparsed)
$ printf '1 + 1\nif (\n'         | hnl-run -   → 2     (unclosed `if` ignored)
```

`stmt.import` evaluates to `null` (an import binds names, yields no
value). Therefore a program whose **first** statement is
`from "host-tensor" import tensor` returns `null` instantly and the rest
of the program — including a 700 MB `do {}` body — never runs:

```sh
printf 'from "host-tensor" import tensor\ndo { tensor.load("/nonexistent-xyz","w") }\n' \
  | hnl-run --project /home/luis/GitHub/handl-lang/host-tensor -
# → "null", exit 0 — the tensor.load error never fires; body never evaluated
```

This is a *known, documented* behavior: `mlopt/src/mlopt/emit.py:2866-2878`
(`_ambient_src`) says verbatim "WITH `--project` the import resolves but
the snippet's value comes back `null` (measured — the project path is for
module files, not stdin snippets)" and strips the exact line
`from "host-tensor" import tensor` before piping to `hnl-run -`.
Emitted **file** artifacts keep the header (`llama1_e2e.hnl:2`), so
`hnl-run --project <host-tensor> artifact.hnl` returns `null` instantly
too — file mode is not immune.

Caveat for Path 2: the import only *resolves* under `--project` (or a
cwd/module-root where `host-tensor` resolves as a module file). Without
it the same input exits 1 with `import "host-tensor": cannot read
module`. `KitRunner` (`emit.py:2948-2980`) never passes `--project`
unless constructed with `project=`, and `cli.py:498` /
`stage.py:793` don't — so Path 2 requires a manual `--project` run or a
caller that opted in.

## Why the incident looked like a stdin flake

`hnl-run -` vs `hnl-run file` differ only in src acquisition. For the
same logical program the outputs cannot differ — so in the incident the
stdin payload must have differed from the file content. The two
byte-level explanations, both reproducing "instant `null`, exit 0":

1. **stdin was empty** (EOF before any bytes — harness/redirect issue).
   Path 1. No guard anywhere distinguishes "empty program" from
   "program evaluated to null".
2. **stdin began with a null-valued first statement** — most plausibly
   the unstripped `from "host-tensor" import tensor` emit header under
   `--project`, or any other leading statement that yields `null`
   (`do {}` also yields `null`). Path 2.

A third, non-reproduced possibility: nondeterminism *inside* `kit.run`
(desugar/module-loader) returning `Ok(null)` for identical src. Nothing
in the run path suggests this — the pipeline is deterministic per
process — but it cannot be excluded for the single observed event.

## Minimal repro commands

```sh
B=/home/luis/GitHub/handl-lang/graphopt/target/debug/hnl-run

# Path 1 — empty stdin:
printf '' | "$B" -
# → null   (exit 0, instant)

# Path 2 — import-header first statement + --project (body never runs):
printf 'from "host-tensor" import tensor\ndo { tensor.load("/nonexistent-xyz","w") }\n' \
  | "$B" --project /home/luis/GitHub/handl-lang/host-tensor -
# → null   (exit 0, instant; no tensor.load error)

# Control — same program without the header computes/errors honestly:
printf 'do { tensor.load("/nonexistent-xyz","w") }\n' | "$B" -
# → hnl-run: Engine { … tensor.load: cannot read … }  (exit 1)
```

## Suspicious code paths (for whoever picks this up)

- `packages/mlopt-kit/src/bin/hnl-run.rs:59-67` — no empty-input check;
  `""` flows into `kit.run` and prints `null` as a *successful* result.
  A `src.trim().is_empty()` → usage/IO error (exit 2) would close Path 1.
- `tool-parse-pratt/src/lib.rs:769-811` — `parse` evaluates exactly
  one `expression_inner`; trailing top-level statements are silently
  dropped (lexed but unparseable content after stmt 1 is ignored —
  `1+1\nif (` → `2`). If multi-statement modules are meant to work,
  this is the bug; the module loader's "module parses several statements
  into one arena" comment (line ~629) suggests they were intended to.
- `exec-tree-walk/src/lib.rs:3914-3916` — empty AST →
  `TaggedValue::Null` rather than an error; combined with the above this
  is what turns "empty stdin" into a JSON `null` success.
- `mlopt/src/mlopt/emit.py:2866-2878` (`_ambient_src`) — strips only the
  exact line `from "host-tensor" import tensor`; any spelling drift
  (whitespace, `\r\n`, reordered/extra imports) leaves a first statement
  that either errors (no `--project`) or yields `null` (with it).
- Downstream is honest: `KitRunner.run` → `json.loads("null")` → `None`,
  and `cli.py:517` treats non-dict output as a gate failure — so a `null`
  can't silently *pass* verify, but it does surface as a confusing
  "not a tensor" decline rather than "hnl-run got empty input".

## Verification performed

- `hnl-run llama1_verify_prog.hnl` (file mode, cwd `/home/luis/tmp` so
  `tensor.load` resolves the sidecar): ran ~4 min, exit 0, 78 657 646-byte
  `{"__tensor__": …}` JSON — matches `llama1_hnl_out_v3.json` size.
- All Path-1/Path-2 repros above run against the same debug binary.
- No source files were modified (read-only investigation).
