# JavaScript Backend

## Context

Handl programs currently run only via the Rust tree-walking evaluator (`handl-plugin-engine-exec-tree-walk`). This limits handl to environments where a Rust binary can run. There is no way to:

- Run handl in the browser
- Run handl in Node.js
- Run handl in edge workers (Cloudflare Workers, Vercel Edge)
- Share handl programs as standalone JS bundles

A JavaScript backend would compile handl arrows to JavaScript, enabling:

- **Web playground** — run handl in the browser without a server
- **Node.js CLIs** — distribute handl programs as npm packages
- **Edge deployment** — compile handl to JS for serverless platforms
- **LSP in the browser** — run the language server in a web worker (for Monaco/CodeMirror editors)

The platform's data-driven architecture makes this feasible. The AST is defined in `ast.yaml` — the JS backend is a new target that walks the same AST and emits JavaScript instead of interpreting it.

## Decision

Build a JavaScript backend as a new target crate: `handl-target-js`. It compiles handl AST nodes to JavaScript source code.

### Architecture

```
.hnl source
    │
    ▼
  Kit (lex + parse)          ← data-driven, same as always
    │
    ▼
  AST (handl-platform AstNode)
    │
    ▼
  handl-target-js             ← new crate, walks AST → emits JS
    │
    ▼
  JavaScript source code
    │
    ▼
  Node.js / Browser / Edge
```

The backend is a `Plugin` that contributes a `Target` operation. It implements the same `Behavior<Target>` interface as any other target. The host (`handl-host-cli`) loads it alongside the existing engines.

### Compilation strategy

**Direct AST-to-JS translation.** No intermediate representation. Each AST node kind maps to a JavaScript expression or statement:

| Handl AST Node                               | JavaScript Output                   |
| -------------------------------------------- | ----------------------------------- |
| `function` (arrow lambda `x ~> body`)        | `(x) => body`                       |
| `binary.and`                                 | `a && b`                            |
| `binary.or`                                  | `a \|\| b`                          |
| `binary.add`                                 | `a + b`                             |
| `binary.eq`                                  | `a === b`                           |
| `function_call` (arrow application `f -< x`) | `f(x)`                              |
| `object`                                     | `{ key: value }`                    |
| `array`                                      | `[a, b, c]`                         |
| `match`                                      | `switch` or pattern-matching helper |
| `loop`                                       | `while` with state object           |
| `do` block (monadic bind `x <- f; rest`)     | `f(x).then(x => rest)` or generator |
| `returnA`                                    | identity / return                   |
| `arr("name")`                                | lookup in stdlib JS module          |
| `first`                                      | `[f(a), b]`                         |
| `choice`                                     | conditional dispatch                |
| `comp` (sequential composition `f >>> g`)    | `(x) => g(f(x))`                    |

### Arrow combinators as JS functions

The handl standard library combinators compile to plain JS functions:

```javascript
// arr("name") → load from stdlib
const arr = (name) => stdlib[name];

// f >>> g → function composition
const comp = (f, g) => (x) => g(f(x));

// first(f) → apply f to first element of pair
const first =
  (f) =>
  ([a, b]) => [f(a), b];

// loop(state, arms) → iterative loop with state
const loop = (initState, arms) => (input) => {
  let state = { ...initState, input };
  while (true) {
    const result = arms(state);
    if (result.done) return result.value;
    state = result.state;
  }
};

// do block → monadic bind chain
const doBlock =
  (...steps) =>
  (input) => {
    let value = input;
    for (const step of steps) {
      value = step(value);
    }
    return value;
  };
```

### Loop compilation

Handl's `loop` is the most complex construct. It compiles to a JavaScript `while` loop with a state object:

```
--- Handl
loop {
  remaining: input,
  acc: []
} (
  []: returnA -< { sorted: state.acc },
  [h, ...t]: do {
    r <- insert -< { elem: h, acc: state.acc };
    returnA -< { state: { remaining: t, acc: r } }
  }
)
```

```javascript
(input) => {
  let state = { remaining: input, acc: [] };
  while (true) {
    if (state.remaining.length === 0) {
      return { sorted: state.acc };
    }
    const [h, ...t] = state.remaining;
    const r = insert({ elem: h, acc: state.acc });
    state = { remaining: t, acc: r };
  }
};
```

### Pattern matching

Handl's `match` compiles to a JS function with type checks:

```javascript
const match = (value, ...cases) => {
  for (const [pattern, body] of cases) {
    const bindings = matchPattern(pattern, value);
    if (bindings !== null) return body(bindings);
  }
  throw new Error("Non-exhaustive match");
};
```

Pattern matching helpers (`matchPattern`, array destructuring, object destructuring) are emitted as a small runtime prelude (~50 lines of JS).

### Runtime prelude

The compiled JS includes a small prelude (~100 lines) containing:

- `comp`, `first`, `choice`, `loop`, `doBlock`, `returnA` — combinator implementations
- `match`, `matchPattern` — pattern matching helpers
- `arr` — stdlib lookup
- Spread/rest helpers for array destructuring

The prelude is prepended to the output. It's tree-shakeable — unused functions are removed by bundlers.

### Output format

```bash
# Compile a single file
handl compile --target js arrows/fn/map.hnl -o map.js

# Compile a directory (produces a single bundle)
handl compile --target js arrows/ -o bundle.js

# Compile with the prelude inlined (standalone)
handl compile --target js --standalone arrows/fn/map.hnl -o map.js

# Compile without prelude (for use with a bundler)
handl compile --target js --no-prelude arrows/fn/map.hnl -o map.js
```

### Source maps

The backend emits source map comments linking JS output back to `.hnl` source. This enables:

- Stack traces pointing to `.hnl` lines
- Debugger breakpoints in `.hnl` source (with source map support)

Source maps are generated using the `sourcemap` crate (pure Rust, no system deps).

### Integration with existing infrastructure

The JS backend is a new `handl-target-js` crate that:

1. Implements `Plugin` from `handl-platform`
2. Contributes a `Behavior<Target>` for the `Target` operation
3. Walks `AstNode` trees and emits JS strings
4. Uses the same `Kit` for parsing (no separate parser)

The host loads it via the kit manifest, same as any other plugin.

### What about refinement types?

The JS backend **does not encode refinement types at runtime**. Refinement types are a compile-time analysis — they guarantee correctness but don't change runtime behavior. The compiled JS is untyped (or optionally typed with JSDoc comments for TypeScript checking).

This is consistent with how the Rust evaluator works: it evaluates the program without checking refinements at runtime. Refinements are checked by the LSP server / typechecker before execution.

Optional: emit JSDoc type annotations from refinement types for TypeScript-aware tooling:

```javascript
/**
 * @param {number[]} input
 * @returns {number[]}
 * @refinement Sorted(input) → Sorted(output)
 */
const sort = (input) => { ... };
```

## Alternatives Considered

### Alternative A: WASM backend

Compile handl to WebAssembly instead of JavaScript.

**Rejected because:**

- WASM is harder to debug (no source maps, no readable output).
- WASM can't directly manipulate the DOM or call JS APIs.
- JS is more portable (runs in more environments than WASM).
- A WASM backend can be added later by compiling the Rust evaluator to WASM (different approach, same result).

**However:** for the LSP-in-browser use case, compiling the existing Rust server to WASM is more practical than re-implementing the server in JS. The JS backend is for running handl _programs_, not the language tooling.

### Alternative B: TypeScript backend

Emit TypeScript instead of JavaScript.

**Rejected because:**

- TypeScript adds type system complexity that doesn't map cleanly to handl's refinement types.
- TypeScript can't be run directly in browsers or edge workers (requires compilation).
- JSDoc annotations give TypeScript checking without emitting TypeScript.
- Can be added later as a flag (`--target ts`) on top of the JS backend.

### Alternative C: Compile through LLVM to JS (emscripten)

Compile the Rust evaluator to JS via emscripten.

**Rejected because:**

- Produces a large JS bundle (includes the entire Rust runtime).
- No readable output — defeats the purpose of a JS backend (debugging, interop, inspection).
- Slow startup (WASM initialization).
- The direct AST-to-JS approach produces small, readable, fast-starting output.

## Consequences

### Positive

- **Run handl anywhere JS runs** — browsers, Node.js, edge workers, Deno, Bun
- **Web playground** — compile handl to JS in the browser, no server needed
- **npm packages** — distribute handl programs as JS bundles
- **Readable output** — developers can inspect and debug the generated JS
- **Source maps** — stack traces and debugging point back to `.hnl` source
- **Small output** — direct AST-to-JS produces minimal code (no runtime overhead beyond the ~100-line prelude)
- **No new analysis** — reuses the existing AST and kit infrastructure

### Negative

- **Pattern matching is not native to JS** — requires a runtime helper (`matchPattern`). This adds overhead and complexity to the output. Mitigated by the small prelude and the fact that most JS engines optimize it well.
- **Loop semantics differ** — handl loops are structural (pattern-matched arms); JS loops are imperative (`while`). The compilation is correct but the generated code doesn't look like idiomatic JS. This is expected — handl is not JS.
- **No refinement checking at runtime** — if a program is compiled and run with inputs that violate the refinement preconditions, the JS will not catch the violation. This is the same as the Rust evaluator. Refinement checking is a compile-time concern.
- **`sourcemap` crate dependency** — adds one crate for source map generation. Minimal impact (pure Rust).

### Neutral

- **No impact on the LSP server or editor extensions** — the JS backend is a separate target crate.
- **No impact on the Rust evaluator** — both backends coexist. The user chooses which target to compile to.

## Status

Proposed
