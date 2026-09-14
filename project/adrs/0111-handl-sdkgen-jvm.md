# ADR-0111: Handl SDK Generator — JVM Backend

## Status

Proposed.

## Context

[ADR-0107 — Handl SDK Generator](./0107-handl-sdk-generator.md) established a
backend-agnostic IR (`SdkgenModule`) and a port-adapter emitter architecture. The
first concrete backend is C / libclang. The next high-priority backend is the
JVM, which lets Handl call Java libraries and native Java classes through JNI.

## Decision

Add a `plugin-sdkgen-backend-jvm` crate that parses Java class / method
signatures (directly or from `.class` / `.jar` metadata) and produces
`SdkgenModule` records.

- The backend may start by consuming a list of Java method signatures or a
  reflection dump; later it can read `.class` files or use a lightweight parser.
- The `client_package` for the generated adapter is `jvm`.
- The adapter uses `jvm.call` / `jvm.exception` primitives (a new `jvm` client
  package similar to `ffi`) to invoke methods and translate exceptions into
  `Result<_, JvmException>`.
- Map common Java types to Handl types:
  - `int` / `Integer` → `Int` (`i32`)
  - `long` / `Long` → `Int` (`i64`)
  - `double` / `Double` → `Float` (`f64`)
  - `String` → `String` (`jstring`)
  - `byte[]` / `ByteBuffer` → `Bytes` (`jbyte_array`)
  - Object references → `Handle` (`jobject`)
  - `void` → `()`
- Support class-name to package mapping, method overloads via name mangling, and
  static vs instance method dispatch.

## Consequences

- Handl can consume the huge ecosystem of JVM libraries.
- A `jvm` client package must be designed and implemented, paralleling `ffi`.
- Overloaded Java methods require a name-mangling scheme in the port.
- This backend is higher priority than Bukkit / Spigot / Paper (ADR-0112) and
  Windows / macOS C support (ADR-0110), because it is a prerequisite for the
  Minecraft server backends.

## References

- [ADR-0107 — Handl SDK Generator](./0107-handl-sdk-generator.md)
- `crates/plugin-sdkgen`
