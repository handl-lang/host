# ADR-0115: Handl SDK Generator — TypeScript Declaration (.d.ts) Backend

## Status

Proposed.

## Context

[ADR-0107 — Handl SDK Generator](./0107-handl-sdk-generator.md) established a
backend-agnostic `SdkgenModule` IR and a port-adapter emitter. The C backend
(`plugin-sdkgen-backend-c`) is implemented, and the WebIDL/JS backend
([ADR-0113](./0113-handl-sdkgen-webidl-js.md)) is the next high-priority target
for Web platform APIs.

Web **platform** APIs are typically described by WebIDL, but the bulk of the web
ecosystem — libraries, UI frameworks, and server runtimes — is distributed as
NPM packages with TypeScript declaration files (`.d.ts`). Those declarations are
already the de-facto machine-readable interface contract for JavaScript code.

The existing `js` client package
(`handl-packages/js/js.hnl`) provides the runtime primitives (`js.call`,
`js.error`) that any generated JS adapter needs. The missing piece is a generator
that turns `.d.ts` descriptions into the same port/adapter Handl source that the
other SDKgen backends produce.

## Decision

Add a `plugin-sdkgen-backend-dts` crate that parses TypeScript declaration
files and produces `SdkgenModule` records.

- The backend consumes one or more `.d.ts` files (e.g. from an NPM package's
  `types` field, from DefinitelyTyped, or from a project bundle).
- The `client_package` for the generated adapter is `js`, reusing the primitives
  from [ADR-0113](./0113-handl-sdkgen-webidl-js.md).
- The backend maps `.d.ts` concepts to Handl types and arrows:

  | TS concept                           | Handl port concept     | Adapter descriptor                  | Notes                                                                                               |
  | ------------------------------------ | ---------------------- | ----------------------------------- | --------------------------------------------------------------------------------------------------- |
  | `number`                             | `Int` or `Float`       | `js_number` / `js_int` / `js_float` | heuristically disambiguated by name and JSDoc; overridable in `.sdkgen.yaml`                        |
  | `string`                             | `String`               | `js_string`                         |                                                                                                     |
  | `boolean`                            | `Bool`                 | `js_bool`                           |                                                                                                     |
  | `Array<T>` / `T[]`                   | `Array`                | `js_array:<el>`                     | recursive descriptor                                                                                |
  | `Uint8Array` / `Buffer`              | `Bytes`                | `js_bytes`                          |                                                                                                     |
  | `object` / `any` / interface / class | `Handle`               | `js_object`                         | opaque reference                                                                                    |
  | interface / class instance           | `Obj` / `Handle`       | `js_object`                         | fields are not directly exposed unless the adapter is read-only                                     |
  | `void`                               | `()`                   | `void`                              |                                                                                                     |
  | `undefined` / optional `?`           | `Option`               | `js_optional:<inner>`               |                                                                                                     |
  | `null` / `T \| null`                 | `Option`               | `js_nullable:<inner>`               |                                                                                                     |
  | `Promise<T>`                         | graded async arrow     | `js_promise:<inner>`                | mapped to a `Task`/`Async` arrow once async grades are stable; v1 can emit a callback-style adapter |
  | function                             | arrow                  | named `export`                      | overloaded signatures get deterministic name mangling                                               |
  | callback type                        | function argument      | `js_callback:<arity>`               | emitted as a Handl arrow marshalled to a JS function                                                |
  | `enum`                               | `Int` + constants      | `js_enum:<name>`                    |                                                                                                     |
  | `const` / `readonly`                 | pure value or constant | `js_const`                          | exported as a zero-argument arrow or a typed value                                                  |

- The output layout follows the established pattern from ADR-0107:
  - **Port** — abstract `export` arrows and types, platform-agnostic.
  - **Adapter** — concrete `export ... :=` arrows that call `js.call` and
    `js.error`.
- `.d.ts` overloads and optional arguments use the same name-mangling / defaulting
  scheme as the WebIDL backend, with deterministic naming (`<module>_<fn>_<arity>`
  or a selected canonical overload when disambiguation is ambiguous).
- A per-package `.sdkgen.yaml` provides overrides for information that cannot be
  recovered from `.d.ts` alone:
  - default vs. named exports,
  - mutable classes and stateful methods,
  - custom marshalling for unusual types,
  - side-effecting module imports,
  - whether a function throws (maps to `Result<_, JsError>`) or returns a raw
    value.

### Example: using an NPM package from Handl

A function-oriented package like `express` ships `.d.ts` declarations. Running
`hnl sdkgen dts` over them produces a port and a `js` adapter, and the resulting
Handl source looks like any other imported module:

```handl
from "npm:express" import { express, get, listen, response_send }

app = express()

get(app, "/", (req, res) ~[JS]~> do {
  response_send(res, "hello")
})

listen(app, 3000)
```

The generated adapter is also ordinary Handl (simplified):

```handl
export express: () ~[JS]~> Express :=
  () ~> call("express", "", [], [], "js_object", false)

export get: (Express, String, Handler) ~[JS]~> () :=
  (app, path, handler) ~> call("express", "get", [app, path, handler], ["js_object", "js_string", "js_callback:2"], "void", false)
```

`from "npm:express"` is not special syntax — it is simply the generated package
path that `hnl sdkgen dts` emitted from the `.d.ts` files plus any
`.sdkgen.yaml` overrides.

### Function libraries vs. component frameworks

This works well for function/value-oriented packages: Express, Lodash, Axios,
`fs`, `crypto`, etc.

It does **not** automatically make React, Vue, or Svelte usable the way they are
in JavaScript. `.d.ts` files do not contain JSX, and the component/reactivity
model is framework-specific. Those libraries are still reachable at the low
level (`createElement`, `useState`, etc.), but a real "React in Handl"
experience requires an additional Handl-side component/reactivity layer above
the generated `js` adapters.

### Scope and limitations

The first version targets the subset of `.d.ts` that is sufficient for NPM
libraries and server frameworks: modules, namespaces, interfaces, type aliases,
function and method declarations, constants, enums, and callback types.

Out of scope for the first version:

- Generics other than monomorphization of a few common instantiations
  (`Array<string>`, `Promise<number>`, etc.).
- Conditional, mapped, template-literal, and recursive conditional types.
- JSX/component view syntax and `.hfc` packaging — these are not part of
  `.d.ts` parsing; see ADR-0116 and ADR-0117.
- Class inheritance, `abstract` classes, and `protected`/`private` members.
- Module augmentation and ambient module declarations.

These are not blocked by the architecture; they are intentionally deferred to
keep the first backend focused on function/value interop.

### Parser choice

The backend wraps a `.d.ts` parser behind the `SdkgenBackend` trait. The first
implementation candidate is a Rust-based TypeScript parser (e.g. `swc`,
`oxc_parser`, or `tree-sitter-typescript`). The parser crate is a private
dependency of `plugin-sdkgen-backend-dts`; changing it does not affect the
public `SdkgenBackend` API or the emitted Handl source. A parser evaluation and
selection step is part of Phase 0.

## Consequences

- The large JavaScript ecosystem becomes reachable from Handl without manual
  adapter authorship for every package.
- Component ergonomics (`.hfc` and `view` syntax) are handled separately by
  ADR-0116 and ADR-0117; the `.d.ts` backend only supplies the low-level
  `npm:<package>` adapter that those layers consume.
- `.d.ts` is richer and more heterogeneous than WebIDL or C headers; the backend
  will need pragmatic subsetting and a generous override mechanism.
- Structural typing and advanced TS features do not map one-to-one to Handl's
  nominal/arrow model; documentation must set expectations about the supported
  subset.
- The WebIDL and `.d.ts` backends share the same `js` client package and adapter
  pattern, so fixes and improvements to `js.call`/`js.error` benefit both.
- Tooling can generate a `from "npm:<package>" import *` style surface, but the
  underlying mechanism is still SDKgen + the `js` client; the runtime import path
  is determined by the generated adapter, not by Handl's module loader.

## References

- [ADR-0107 — Handl SDK Generator](./0107-handl-sdk-generator.md)
- [ADR-0113 — Handl SDK Generator: WebIDL / JS Backend](./0113-handl-sdkgen-webidl-js.md)
- `handl-packages/js/js.hnl` — the JS client package
- `crates/plugin-sdkgen` — the SDK generator core
