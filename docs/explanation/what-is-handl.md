# What is handl?

handl is a small, functional, data-oriented language designed for safe, portable computation across environments.

## Core idea

Programs in handl are expressions that transform JSON-like data. There are no side effects in the language itself; interaction with the outside world is delegated to the *runtime* through modules. This separation makes handl scripts easy to test, reason about, and execute in different contexts.

## Why handl?

- **JSON native**: all values are JSON values — strings, numbers, booleans, null, arrays, and objects.
- **Capability oriented**: modules like `fs`, `env`, and `http` are injected by the runtime, so the same script can run safely in a browser, server, or embedded device.
- **Functional**: no mutable variables, explicit data flow, and composable functions.
- **Embeddable**: the handl VM is small and built in Rust, making it easy to embed in other systems.

## Where handl fits

handl is a good fit for:

- Data processing scripts that must run in multiple environments.
- Configuration or policy logic that benefits from sandboxed execution.
- Embedded expressions in larger applications.

It is not a general-purpose systems language: it deliberately delegates I/O, concurrency, and platform details to the runtime.
