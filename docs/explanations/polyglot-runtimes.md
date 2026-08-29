# `handl` and other polyglot runtimes

`handl` is **not the first polyglot runtime**. Several systems already run multiple languages or syntaxes in the same process. What makes `handl` different is the _plugin-driven, self-hosting composer_ model.

## Existing polyglot runtimes

| runtime                | how it is polyglot                                                                                                                   |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| **GraalVM / Truffle**  | Languages are implemented as Truffle interpreters on a shared JVM. They share a common `Polyglot` API and can pass values directly.  |
| **.NET CLR**           | C#, F#, VB, and others compile to Common Intermediate Language (CIL) and run on the same VM.                                         |
| **JVM**                | Java, Kotlin, Scala, Clojure, and others share the Java Virtual Machine and bytecode.                                                |
| **BEAM**               | The Erlang/Elixir runtime also hosts Luerl, Clojure-erl, and other languages.                                                        |
| **WebAssembly**        | C, C++, Rust, Go, and many others compile to WASM and run in the same sandbox.                                                       |
| **inauguration / .in** | Multi-frontend parser that uses Tree-sitter to ingest Swift, C, Rust, Go, Python, and more, then lowers them into a unified Core IR. |
| **inlang / Fink**      | Similar idea: multiple language fronts, one intermediate representation.                                                             |
| **Cornell Polyglot**   | An extensible Java front-end for Java dialects.                                                                                      |

These systems are mature and well-tested. They prove that polyglot execution is possible.

## How `handl` is different

The existing systems are mostly **"one big VM, many languages compiled to a common IR."** `handl` takes a different path:

1. **Languages are plugins.**
   A new syntax is not a compiler back-end or a VM target. It is a set of `Behavior<Parser>`, `Behavior<Lexer>`, and `Behavior<Format>` rules contributed by a plugin like `handl-plugin-yaml-syntax` or `handl-plugin-python-syntax`. The rules produce and consume the same `AstNode` type.

2. **The host is the runtime composer.**
   The `Host` loads plugins, resolves dependencies, and assembles the language. It does not define the language itself. The language is the _composition_ of the loaded plugins.

3. **The language can host and spawn hosts.**
   Because the `Host` is minimal and plugin-based, a `handl` program can eventually create and configure new `Host` instances. This makes `handl` able to describe its own runtime composition.

4. **Sandboxing by design.**
   Capabilities such as `contribute`, `register-plugin`, and `execute` can be tracked as `handl` effects. A module that does not declare a capability cannot use it, even if it is loaded.

## What is arguably new

It would be inaccurate to claim that `handl` is the first polyglot runtime. It is more precise to say:

> `handl` may be the first polyglot runtime built around a universal `Plugin` model where the parser, lexer, formatter, and typechecker are all composable rule sets, and where the host itself can be written and spawned from within the language.

That model — a `Plugin` graph that assembles languages, and a language that can assemble its own runtime — is the novel part. In short, `handl` aims to be a **self-describing, self-hosting, capability-sandboxed polyglot composer**.
