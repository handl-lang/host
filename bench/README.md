# bench — handl vs other languages

Cross-language benchmarks comparing **handl** against **bun**, **node**,
**python**, **rust**, and **c** on identical algorithms.

The focus is **handl vs bun** (per the original request), but the suite
includes node, python, rust, and c as reference points:

- **bun / node** — JIT-compiled JavaScript. The direct comparison target.
- **python** — familiar interpreted baseline.
- **rust** — the language handl is built in; shows the host's native ceiling.
- **c** — the bare-metal ceiling.

## Quick start

```bash
# from platform/ root:

# 1. build hnl in release mode + compile rust/c task binaries
./bench/run.sh --build

# 2. run all benchmarks (10 e2e runs each + heavy-work single runs)
./bench/run.sh

# faster: fewer runs
./bench/run.sh --runs 5

# single task
./bench/run.sh --task fibonacci

# single mode
./bench/run.sh --mode e2e
./bench/run.sh --mode heavy
```

Results are printed as tables to stdout and written as CSV to
`bench/results/` (gitignored).

## Tasks

| Task         | What it tests                     | e2e params  | heavy params |
| ------------ | --------------------------------- | ----------- | ------------ |
| `fibonacci`  | tight iterative arithmetic loop   | fib(35)     | fib(38)      |
| `ackermann`  | deep recursion, function dispatch | ack(2,5)    | ack(3,3)     |
| `cold-start` | process startup + runtime init    | hello world | —            |

Each task directory (`tasks/<name>/`) contains identical algorithms in every
language. See each task's `README.md` for details.

## Methodology

### Two measurement modes

1. **End-to-end** — wall clock per invocation, N runs (default 10), reporting
   min / median / stdev. This includes process startup, runtime
   initialization, computation, and shutdown. This is what a user actually
   feels when they run `hnl run file.hnl` or `bun run file.js`.

2. **Heavy-work** — single run with a larger input where startup is
   negligible. This isolates raw compute speed from startup overhead.

### Fairness

- **Same algorithm** in every language. No language gets a "clever" version.
  The fibonacci is iterative everywhere; ackermann is recursive everywhere.
- **Same input** per task per mode. The gap between languages IS the data.
- **Release builds** for compiled languages. The harness prefers
  `target/release/hnl` over debug. Use `--build` to build everything first.
- **Correctness checked** — each run's output is verified against the expected
  value. A "NO" in the `ok` column means the output didn't match; the timing
  is still reported but flagged.
- **No warmup** — each invocation is a cold process. This is intentional for
  end-to-end mode (we're measuring cold start + compute together). For
  heavy-work mode, the single long run makes warmup irrelevant.

### What this is NOT

- **Not a JIT benchmark.** handl's canonical evaluator is a tree-walk
  interpreter. bun/node have JIT compilers. The gap on tight loops and
  recursion is expected and is useful data for motivating the cranelift
  backend (`plugin-engine-eval-cranelift`), not a bug.
- **Not a standard library benchmark.** Programs use only language builtins
  (arithmetic, print, process.argv). No external packages.
- **Not a startup microbenchmark only.** Cold-start is one task; the others
  do real work.

## Directory structure

```
bench/
  README.md            this file
  run.py               harness (Python)
  run.sh               convenience wrapper
  .gitignore           ignores results/, bin/, _generated.hnl
  tasks/
    fibonacci/
      README.md        algorithm, params, expected outputs
      handl.hnl.template   __ARG1__ placeholder, harness substitutes
      js.js            shared by bun and node
      python.py
      rust.rs
      c.c
    ackermann/
      ...same layout...
    cold-start/
      handl.hnl        no template (no args)
      js.js
      python.py
      rust.rs
      c.c
  results/             gitignored — e2e.csv, heavy.csv
  bin/                 gitignored — compiled rust/c binaries
```

## Adding a new task

1. Create `tasks/<name>/` with source files for each language.
2. For handl, use a `handl.hnl.template` with `__ARG1__`, `__ARG2__`, etc.
   placeholders (or `handl.hnl` if no args needed).
3. Add a `Task` entry to the `TASKS` list in `run.py`.
4. Add a `README.md` in the task dir documenting the algorithm, params, and
   expected outputs.

## Adding a new language

1. Add a `Language` entry to the `LANGUAGES` list in `run.py`.
2. Add a `source_file` for each task directory.
3. If the language needs a build step, set `build_cmd` and `binary_template`.
4. Add a `runner` case in `make_run_cmd()` if it uses a new invocation pattern.
